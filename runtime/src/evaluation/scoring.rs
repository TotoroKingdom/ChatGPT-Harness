use super::case::{
    ArgumentConstraint, EvalCaseV1, FinalResponseAssertion, FinalStateAssertion, ResponseMatchKind,
    RuntimeOutcome, SelectionStep, StepRepeat,
};
use super::fixture::{safe_relative, workspace_file_exists, workspace_file_sha256};
use super::manifest::{normalize_lf, sha256_hex};
use super::privacy::sanitize_bounded_text;
use super::{EvalError, EvalResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssertionEvidence {
    pub(crate) kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    pub(crate) pass: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) expected_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) actual_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) detail: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FinalStateEvidence {
    pub(crate) assertions: Vec<AssertionEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) final_response: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_pass: Option<bool>,
    pub(crate) task_completion_pass: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolCallRecord {
    pub(crate) model_tool_call_id: String,
    pub(crate) tool_name: String,
    pub(crate) schema_valid: bool,
    pub(crate) semantic_valid: bool,
    pub(crate) forwarded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_outcome: Option<RuntimeOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) safe_error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) approval_decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) approval_request_id: Option<String>,
    pub(crate) argument_evidence: ArgumentEvidence,
    #[serde(default)]
    pub(crate) evidence_verified: bool,
    #[serde(default)]
    pub(crate) unnecessary: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArgumentEvidence {
    pub(crate) sha256: String,
    pub(crate) parseable: bool,
    pub(crate) keys: Vec<String>,
    pub(crate) bounded_preview: String,
}

pub(crate) fn bounded_argument_evidence(raw: &str) -> ArgumentEvidence {
    let parsed = serde_json::from_str::<Value>(raw).ok();
    let keys = parsed
        .as_ref()
        .and_then(Value::as_object)
        .map(|object| {
            object
                .keys()
                .take(16)
                .map(|key| sanitize_bounded_text(key, None, 64))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let bounded_preview = match &parsed {
        Some(Value::Object(object)) => {
            let mut parts = Vec::new();
            for (key, value) in object.iter().take(16) {
                let kind = match value {
                    Value::Array(_) => "<array>",
                    Value::Object(_) => "<object>",
                    Value::String(_) => "<string>",
                    Value::Number(_) => "<number>",
                    Value::Bool(_) => "<boolean>",
                    Value::Null => "<null>",
                };
                let key = sanitize_bounded_text(key, None, 64);
                parts.push(format!("\"{key}\":\"{kind}\""));
            }
            format!("{{{}}}", parts.join(","))
        }
        Some(_) => "<non-object-json>".into(),
        None => "<invalid-json>".into(),
    };
    ArgumentEvidence {
        sha256: sha256_hex(raw.as_bytes()),
        parseable: parsed.is_some(),
        keys,
        bounded_preview: bounded_preview.chars().take(512).collect(),
    }
}

pub(crate) fn validate_schema(schema: &Value, arguments: Option<&Value>) -> bool {
    let Some(arguments) = arguments else {
        return false;
    };
    match jsonschema::validator_for(schema) {
        Ok(validator) => validator.is_valid(arguments),
        Err(_) => false,
    }
}

pub(crate) fn score_selection(case: &EvalCaseV1, calls: &mut [ToolCallRecord]) -> bool {
    if calls.is_empty() {
        return false;
    }
    let allowed_tools = case
        .selection_oracle
        .strategies
        .iter()
        .flat_map(|strategy| strategy.steps.iter().map(|step| step.tool.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    if calls
        .iter()
        .any(|call| !allowed_tools.contains(call.tool_name.as_str()))
    {
        for call in calls.iter_mut() {
            call.unnecessary = true;
        }
        return false;
    }
    let names = calls
        .iter()
        .map(|call| call.tool_name.as_str())
        .collect::<Vec<_>>();
    let mut matched = None;
    for strategy in &case.selection_oracle.strategies {
        if let Some(consumed) = match_steps(&strategy.steps, 0, &names, 0, &mut Vec::new()) {
            matched = Some(consumed);
            break;
        }
    }
    let Some(consumed) = matched else {
        for call in calls.iter_mut() {
            call.unnecessary = true;
        }
        return false;
    };
    for (index, call) in calls.iter_mut().enumerate() {
        call.unnecessary = !consumed.contains(&index);
    }
    true
}

fn match_steps(
    steps: &[SelectionStep],
    step_index: usize,
    names: &[&str],
    name_index: usize,
    consumed: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    if step_index == steps.len() {
        return Some(consumed.clone());
    }
    let step = &steps[step_index];
    match step.repeat {
        StepRepeat::One => {
            if name_index < names.len() && names[name_index] == step.tool {
                consumed.push(name_index);
                if let Some(result) =
                    match_steps(steps, step_index + 1, names, name_index + 1, consumed)
                {
                    return Some(result);
                }
                consumed.pop();
            }
            if step.optional {
                match_steps(steps, step_index + 1, names, name_index, consumed)
            } else {
                None
            }
        }
        StepRepeat::OneOrMore => {
            let mut end = name_index;
            while end < names.len() && names[end] == step.tool {
                end += 1;
            }
            if end == name_index {
                return None;
            }
            let start_len = consumed.len();
            for count in (1..=end - name_index).rev() {
                consumed.truncate(start_len);
                consumed.extend(name_index..name_index + count);
                if let Some(result) =
                    match_steps(steps, step_index + 1, names, name_index + count, consumed)
                {
                    return Some(result);
                }
            }
            consumed.truncate(start_len);
            if step.optional {
                match_steps(steps, step_index + 1, names, name_index, consumed)
            } else {
                None
            }
        }
    }
}

pub(crate) fn score_arguments(
    case: &EvalCaseV1,
    call: &ToolCallRecord,
    arguments: Option<&Value>,
) -> bool {
    let Some(arguments) = arguments else {
        return false;
    };
    case.argument_constraints
        .iter()
        .filter(|constraint| constraint.tool == call.tool_name)
        .any(|constraint| constraint_matches(constraint, arguments))
}

fn constraint_matches(constraint: &ArgumentConstraint, arguments: &Value) -> bool {
    let path = arguments.get("path").and_then(Value::as_str);
    if let Some(expected) = &constraint.path {
        if path != Some(expected.as_str()) {
            return false;
        }
    }
    if !constraint.path_any.is_empty()
        && !path.is_some_and(|value| constraint.path_any.iter().any(|item| item == value))
    {
        return false;
    }
    if let Some(suffix) = &constraint.path_suffix {
        if !path.is_some_and(|value| value.ends_with(suffix)) {
            return false;
        }
    }
    if let Some(expected) = &constraint.content {
        if arguments.get("content").and_then(Value::as_str) != Some(expected.as_str()) {
            return false;
        }
    }
    if let Some(expected) = &constraint.old_text {
        if arguments.get("oldText").and_then(Value::as_str) != Some(expected.as_str()) {
            return false;
        }
    }
    if let Some(expected) = &constraint.new_text {
        if arguments.get("newText").and_then(Value::as_str) != Some(expected.as_str()) {
            return false;
        }
    }
    if let Some(expected) = &constraint.pattern {
        if arguments.get("pattern").and_then(Value::as_str) != Some(expected.as_str()) {
            return false;
        }
    }
    if !constraint.pattern_must_match_any.is_empty() {
        let Some(pattern) = arguments.get("pattern").and_then(Value::as_str) else {
            return false;
        };
        if !constraint
            .pattern_must_match_any
            .iter()
            .any(|path| simple_glob_matches(pattern, path))
        {
            return false;
        }
    }
    true
}

/// Minimal deterministic glob matcher for semantic grading only. Runtime keeps
/// using globset for real execution; this avoids platform separator drift in
/// case ground truth.
pub(crate) fn simple_glob_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.replace('\\', "/").into_bytes();
    let path = path.replace('\\', "/").into_bytes();
    let mut memo = std::collections::HashMap::new();
    glob_dp(&pattern, &path, 0, 0, &mut memo)
}

fn glob_dp(
    pattern: &[u8],
    path: &[u8],
    pattern_index: usize,
    path_index: usize,
    memo: &mut std::collections::HashMap<(usize, usize), bool>,
) -> bool {
    if let Some(value) = memo.get(&(pattern_index, path_index)) {
        return *value;
    }
    let result = if pattern_index == pattern.len() {
        path_index == path.len()
    } else if pattern[pattern_index] == b'*' {
        if pattern.get(pattern_index + 1) == Some(&b'*') {
            let next = if pattern.get(pattern_index + 2) == Some(&b'/') {
                pattern_index + 3
            } else {
                pattern_index + 2
            };
            glob_dp(pattern, path, next, path_index, memo)
                || (path_index < path.len()
                    && glob_dp(pattern, path, pattern_index, path_index + 1, memo))
        } else {
            glob_dp(pattern, path, pattern_index + 1, path_index, memo)
                || (path_index < path.len()
                    && path[path_index] != b'/'
                    && glob_dp(pattern, path, pattern_index, path_index + 1, memo))
        }
    } else if pattern[pattern_index] == b'?' {
        path_index < path.len()
            && path[path_index] != b'/'
            && glob_dp(pattern, path, pattern_index + 1, path_index + 1, memo)
    } else {
        path_index < path.len()
            && pattern[pattern_index] == path[path_index]
            && glob_dp(pattern, path, pattern_index + 1, path_index + 1, memo)
    };
    memo.insert((pattern_index, path_index), result);
    result
}

pub(crate) fn normalize_runtime_outcome(result: Option<&Value>) -> RuntimeOutcome {
    let Some(result) = result else {
        return RuntimeOutcome::ExecutionFailed;
    };
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !is_error {
        return RuntimeOutcome::Success;
    }
    let code = result
        .get("structuredContent")
        .and_then(|value| value.get("code"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    match code {
        "POLICY_DENIED" => RuntimeOutcome::PolicyDenied,
        "APPROVAL_DENIED" => RuntimeOutcome::ApprovalDenied,
        "APPROVAL_EXPIRED" | "APPROVAL_CANCELLED" => RuntimeOutcome::Cancelled,
        "WORKSPACE_VIOLATION" => RuntimeOutcome::WorkspaceViolation,
        "INVALID_ARGUMENTS" | "INVALID_PATTERN" | "INVALID_BASE_PATH" => {
            RuntimeOutcome::InvalidArguments
        }
        "TARGET_NOT_FOUND" | "TARGET_NOT_FILE" | "TARGET_NOT_DIRECTORY" => {
            RuntimeOutcome::TargetNotFound
        }
        "CANCELLED" | "BASH_CANCELLED" => RuntimeOutcome::Cancelled,
        "TIMED_OUT" | "BASH_TIMEOUT" => RuntimeOutcome::TimedOut,
        "EXECUTION_FAILED"
        | "IO_FAILURE"
        | "TARGET_CHANGED"
        | "EDIT_MATCH_NOT_FOUND"
        | "EDIT_MATCH_NOT_UNIQUE"
        | "UNSUPPORTED_CONTENT"
        | "READ_LIMIT_EXCEEDED"
        | "RUNTIME_NOT_INITIALIZED"
        | "RUNTIME_NOT_READY"
        | "TOOL_DISABLED"
        | "APPROVAL_REQUIRED" => RuntimeOutcome::ExecutionFailed,
        "" => RuntimeOutcome::ExecutionFailed,
        _ => RuntimeOutcome::Unknown,
    }
}

pub(crate) fn score_runtime_outcomes(case: &EvalCaseV1, calls: &[ToolCallRecord]) -> (u32, u32) {
    let mut matches = 0;
    let mut total = 0;
    let mut occurrences = std::collections::BTreeMap::<&str, u32>::new();
    for call in calls.iter().filter(|call| call.forwarded) {
        let occurrence = occurrences.entry(call.tool_name.as_str()).or_default();
        *occurrence += 1;
        let expected = case
            .expected_runtime_outcomes
            .iter()
            .find(|expected| expected.tool == call.tool_name && expected.occurrence == *occurrence);
        if let Some(expected) = expected {
            total += 1;
            if call.runtime_outcome == Some(expected.outcome) {
                matches += 1;
            }
        }
    }
    (matches, total)
}

pub(crate) fn score_final_state(
    case: &EvalCaseV1,
    workspace_root: &Path,
    final_response: Option<&str>,
) -> EvalResult<FinalStateEvidence> {
    let mut evidence = FinalStateEvidence::default();
    for assertion in &case.final_state_assertions {
        evidence
            .assertions
            .push(score_assertion(workspace_root, assertion)?);
    }
    let response_pass = case
        .final_response_assertion
        .as_ref()
        .map(|assertion| score_response(assertion, final_response));
    evidence.response_pass = response_pass;
    evidence.final_response =
        final_response.map(|value| sanitize_bounded_text(value.trim(), Some(workspace_root), 256));
    evidence.task_completion_pass =
        evidence.assertions.iter().all(|item| item.pass) && response_pass.unwrap_or(true);
    Ok(evidence)
}

fn score_assertion(root: &Path, assertion: &FinalStateAssertion) -> EvalResult<AssertionEvidence> {
    match assertion {
        FinalStateAssertion::FileExact { path, content } => {
            let target = safe_relative(path)?;
            let actual = fs::read(root.join(&target)).map_err(|error| {
                EvalError::new(
                    "INFRA_ERROR",
                    format!("could not read final-state target: {error}"),
                )
            });
            let expected_hash = sha256_hex(&normalize_lf(content.as_bytes()));
            match actual {
                Ok(bytes) => {
                    let actual_hash = sha256_hex(&normalize_lf(&bytes));
                    Ok(AssertionEvidence {
                        kind: "file_exact".into(),
                        path: Some(path.clone()),
                        pass: actual_hash == expected_hash,
                        expected_sha256: Some(expected_hash),
                        actual_sha256: Some(actual_hash),
                        detail: None,
                    })
                }
                Err(_) => Ok(AssertionEvidence {
                    kind: "file_exact".into(),
                    path: Some(path.clone()),
                    pass: false,
                    expected_sha256: Some(expected_hash),
                    actual_sha256: None,
                    detail: Some("file missing or unreadable".into()),
                }),
            }
        }
        FinalStateAssertion::FileExists { path } => Ok(AssertionEvidence {
            kind: "file_exists".into(),
            path: Some(path.clone()),
            pass: workspace_file_exists(root, path)?,
            expected_sha256: None,
            actual_sha256: None,
            detail: None,
        }),
        FinalStateAssertion::FileAbsent { path } => Ok(AssertionEvidence {
            kind: "file_absent".into(),
            path: Some(path.clone()),
            pass: !workspace_file_exists(root, path)?,
            expected_sha256: None,
            actual_sha256: None,
            detail: None,
        }),
        FinalStateAssertion::Unchanged { path, sha256 } => {
            let actual = workspace_file_sha256(root, path)?;
            Ok(AssertionEvidence {
                kind: "unchanged".into(),
                path: Some(path.clone()),
                pass: actual == *sha256,
                expected_sha256: Some(sha256.clone()),
                actual_sha256: Some(actual),
                detail: None,
            })
        }
    }
}

fn score_response(assertion: &FinalResponseAssertion, response: Option<&str>) -> bool {
    let Some(response) = response else {
        return false;
    };
    match assertion.kind {
        ResponseMatchKind::Equals => response.trim() == assertion.text.trim(),
        ResponseMatchKind::Contains => response.contains(&assertion.text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluation::case::EvalCaseV1;
    use serde_json::json;

    fn load_case(case_id: &str) -> EvalCaseV1 {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
            .join("v4")
            .join("m04-eval")
            .join("cases")
            .join(format!("{case_id}.json"));
        serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap()
    }

    fn record(tool: &str) -> ToolCallRecord {
        ToolCallRecord {
            model_tool_call_id: "call-1".into(),
            tool_name: tool.into(),
            schema_valid: true,
            semantic_valid: true,
            forwarded: true,
            runtime_id: None,
            request_id: None,
            runtime_outcome: Some(RuntimeOutcome::Success),
            safe_error_code: None,
            approval_decision: None,
            approval_request_id: None,
            argument_evidence: bounded_argument_evidence("{\"path\":\"config/service.txt\"}"),
            evidence_verified: false,
            unnecessary: false,
        }
    }

    #[test]
    fn selection_rejects_wrong_tool_and_counts_unnecessary_calls() {
        let case = load_case("m04-c1-01");
        let mut calls = vec![record("glob")];
        assert!(!score_selection(&case, &mut calls));
        assert!(calls[0].unnecessary);

        let mut calls = vec![record("read_file"), record("read_file")];
        assert!(score_selection(&case, &mut calls));
        assert!(!calls[0].unnecessary);
        assert!(calls[1].unnecessary);
    }

    #[test]
    fn selection_matches_path_discovery_and_content_search() {
        let case = load_case("m04-c2-01");
        let mut calls = vec![record("glob"), record("read_file")];
        assert!(score_selection(&case, &mut calls));

        let case = load_case("m04-c3-01");
        let mut calls = vec![record("glob"), record("read_file"), record("read_file")];
        assert!(score_selection(&case, &mut calls));
    }

    #[test]
    fn selection_matrix_covers_every_frozen_strategy() {
        let cases = [
            ("m04-c1-01", vec!["read_file"]),
            ("m04-c2-01", vec!["glob", "read_file"]),
            ("m04-c3-01", vec!["glob", "read_file", "read_file"]),
            ("m04-c4-01", vec!["write", "read_file"]),
            ("m04-c5-01", vec!["read_file", "edit", "read_file"]),
            ("m04-c6-01", vec!["write"]),
            ("m04-c6-02", vec!["edit"]),
            ("m04-c6-03", vec!["read_file"]),
            ("m04-c6-04", vec!["read_file"]),
        ];
        for (case_id, tools) in cases {
            let case = load_case(case_id);
            let mut calls = tools.into_iter().map(record).collect::<Vec<_>>();
            assert!(score_selection(&case, &mut calls), "{case_id}");
            assert!(calls.iter().all(|call| !call.unnecessary), "{case_id}");
        }
    }

    #[test]
    fn selection_rejects_no_call_wrong_tool_and_disallowed_extra_tool() {
        let case = load_case("m04-c1-01");
        assert!(!score_selection(&case, &mut []));

        let mut wrong = vec![record("glob")];
        assert!(!score_selection(&case, &mut wrong));
        assert!(wrong[0].unnecessary);

        let mut extra = vec![record("read_file"), record("write")];
        assert!(!score_selection(&case, &mut extra));
        assert!(extra.iter().all(|call| call.unnecessary));
    }

    #[test]
    fn argument_validity_separates_schema_and_semantics() {
        let case = load_case("m04-c1-01");
        let mut call = record("read_file");
        assert!(score_arguments(
            &case,
            &call,
            Some(&json!({"path": "config/service.txt"}))
        ));
        assert!(!score_arguments(
            &case,
            &call,
            Some(&json!({"path": "other.txt"}))
        ));
        call.tool_name = "glob".into();
        assert!(!score_arguments(
            &case,
            &call,
            Some(&json!({"path": "config/service.txt"}))
        ));
    }

    #[test]
    fn argument_evidence_is_bounded_and_redacted() {
        let evidence = bounded_argument_evidence(
            r#"{"path":"C:\\private\\workspace","content":"sk-live-secret"}"#,
        );
        assert!(evidence.parseable);
        assert_eq!(evidence.keys.len(), 2);
        assert!(!evidence.bounded_preview.contains("private"));
        assert!(!evidence.bounded_preview.contains("sk-live"));
        assert!(evidence.bounded_preview.len() <= 512);
        let invalid = bounded_argument_evidence("not-json");
        assert!(!invalid.parseable);
        assert_eq!(invalid.bounded_preview, "<invalid-json>");
    }

    #[test]
    fn runtime_outcome_normalization_handles_denial_and_unknown() {
        assert_eq!(
            normalize_runtime_outcome(Some(&json!({
                "isError": true,
                "structuredContent": {"code": "POLICY_DENIED"}
            }))),
            RuntimeOutcome::PolicyDenied
        );
        assert_eq!(
            normalize_runtime_outcome(Some(&json!({
                "isError": true,
                "structuredContent": {"code": "SOMETHING_NEW"}
            }))),
            RuntimeOutcome::Unknown
        );
        assert_eq!(
            normalize_runtime_outcome(Some(&json!({
                "isError": true,
                "structuredContent": {"code": "CANCELLED"}
            }))),
            RuntimeOutcome::Cancelled
        );
        assert_eq!(
            normalize_runtime_outcome(Some(&json!({
                "isError": true,
                "structuredContent": {"code": "TIMED_OUT"}
            }))),
            RuntimeOutcome::TimedOut
        );
        assert_eq!(
            normalize_runtime_outcome(Some(&json!({"isError": false}))),
            RuntimeOutcome::Success
        );
    }

    #[test]
    fn final_state_scorer_requires_real_state_not_tool_success() {
        let case = load_case("m04-c4-01");
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("sentinel.txt"),
            "sentinel-m04-c4-01\n",
        )
        .unwrap();
        let evidence = score_final_state(&case, workspace.path(), Some("DONE")).unwrap();
        assert!(!evidence.task_completion_pass);

        std::fs::write(workspace.path().join("report_output.txt"), "report=ok").unwrap();
        let evidence = score_final_state(&case, workspace.path(), Some("DONE")).unwrap();
        assert!(evidence.task_completion_pass);

        let evidence = score_final_state(&case, workspace.path(), Some("not done")).unwrap();
        assert!(!evidence.task_completion_pass);
    }

    #[test]
    fn expected_denial_without_mutation_passes() {
        let case = load_case("m04-c6-01");
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("sentinel.txt"),
            "sentinel-m04-c6-01\n",
        )
        .unwrap();
        let evidence = score_final_state(&case, workspace.path(), Some("BLOCKED")).unwrap();
        assert!(evidence.task_completion_pass);
        std::fs::write(workspace.path().join("blocked.txt"), "should-not-exist").unwrap();
        let evidence = score_final_state(&case, workspace.path(), Some("BLOCKED")).unwrap();
        assert!(!evidence.task_completion_pass);
    }

    #[test]
    fn final_state_golden_matrix_is_independent_from_runtime_success() {
        let c4 = load_case("m04-c4-01");
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("sentinel.txt"),
            "sentinel-m04-c4-01\n",
        )
        .unwrap();
        std::fs::write(workspace.path().join("report_output.txt"), "report=ok").unwrap();
        let good = score_final_state(&c4, workspace.path(), Some("DONE")).unwrap();
        assert!(good.task_completion_pass);

        let mut wrong_selection = vec![record("glob")];
        assert!(!score_selection(&c4, &mut wrong_selection));
        assert!(good.task_completion_pass);

        std::fs::write(workspace.path().join("sentinel.txt"), "changed").unwrap();
        let sentinel_changed = score_final_state(&c4, workspace.path(), Some("DONE")).unwrap();
        assert!(!sentinel_changed.task_completion_pass);

        let c5 = load_case("m04-c5-01");
        let edit_workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(edit_workspace.path().join("settings")).unwrap();
        std::fs::write(
            edit_workspace.path().join("settings/app.txt"),
            "name=demo\nmode=new\nkeep=yes\n",
        )
        .unwrap();
        std::fs::write(
            edit_workspace.path().join("sentinel.txt"),
            "sentinel-m04-c5-01\n",
        )
        .unwrap();
        let edited = score_final_state(&c5, edit_workspace.path(), Some("DONE")).unwrap();
        assert!(edited.task_completion_pass);

        std::fs::write(
            edit_workspace.path().join("settings/app.txt"),
            "name=demo\nmode=new\nkeep=no\n",
        )
        .unwrap();
        let wrong_edit = score_final_state(&c5, edit_workspace.path(), Some("DONE")).unwrap();
        assert!(!wrong_edit.task_completion_pass);

        let c6 = load_case("m04-c6-01");
        let denied_workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            denied_workspace.path().join("sentinel.txt"),
            "sentinel-m04-c6-01\n",
        )
        .unwrap();
        assert!(
            score_final_state(&c6, denied_workspace.path(), Some("BLOCKED"))
                .unwrap()
                .task_completion_pass
        );
        std::fs::write(
            denied_workspace.path().join("blocked.txt"),
            "should-not-exist",
        )
        .unwrap();
        assert!(
            !score_final_state(&c6, denied_workspace.path(), Some("BLOCKED"))
                .unwrap()
                .task_completion_pass
        );
    }

    #[test]
    fn exact_read_requires_exact_response_and_unchanged_sentinel() {
        let case = load_case("m04-c1-01");
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(
            workspace.path().join("sentinel.txt"),
            "sentinel-m04-c1-01\n",
        )
        .unwrap();
        assert!(
            score_final_state(&case, workspace.path(), Some("PORT=4317"))
                .unwrap()
                .task_completion_pass
        );
        assert!(
            !score_final_state(&case, workspace.path(), Some("PORT=0000"))
                .unwrap()
                .task_completion_pass
        );
        std::fs::write(workspace.path().join("sentinel.txt"), "changed").unwrap();
        assert!(
            !score_final_state(&case, workspace.path(), Some("PORT=4317"))
                .unwrap()
                .task_completion_pass
        );
    }

    #[test]
    fn simple_glob_supports_double_star_and_single_star() {
        assert!(simple_glob_matches(
            "**/invoice_q4.txt",
            "archive/2024/invoice_q4.txt"
        ));
        assert!(simple_glob_matches("notes/*.txt", "notes/alpha.txt"));
        assert!(!simple_glob_matches(
            "notes/*.txt",
            "notes/nested/alpha.txt"
        ));
        assert!(!simple_glob_matches(
            "**/other.txt",
            "archive/2024/invoice_q4.txt"
        ));
    }
}
