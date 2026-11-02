use super::{EvalError, EvalResult};
use chatgpt_harness_evidence_store::EvidenceReader;
use harness_control_protocol::{HistoryGetResult, HistoryTerminalState};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub(crate) const META_KEY: &str = "chatgptHarness";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CorrelationKey {
    pub(crate) runtime_id: String,
    pub(crate) request_id: String,
}

pub(crate) fn extract(result: &Value) -> Option<CorrelationKey> {
    let meta = result.get("_meta")?.get(META_KEY)?;
    let runtime_id = meta.get("runtimeId")?.as_str()?.to_owned();
    let request_id = meta.get("requestId")?.as_str()?.to_owned();
    Some(CorrelationKey {
        runtime_id,
        request_id,
    })
}

pub(crate) fn strip_model_metadata(result: &mut Value) {
    if let Some(meta) = result.get_mut("_meta").and_then(Value::as_object_mut) {
        meta.remove(META_KEY);
        if meta.is_empty() {
            if let Some(object) = result.as_object_mut() {
                object.remove("_meta");
            }
        }
    }
}

pub(crate) async fn verify_evidence(
    database: &Path,
    key: &CorrelationKey,
    timeout: Duration,
) -> EvalResult<HistoryGetResult> {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let found = {
            let reader = EvidenceReader::open(database).map_err(|error| {
                EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    format!("could not open M03 evidence: {error}"),
                )
            })?;
            reader
                .get(&key.runtime_id, &key.request_id)
                .map_err(|error| {
                    EvalError::new(
                        "HARNESS_CONTRACT_ERROR",
                        format!("could not query M03 evidence: {error}"),
                    )
                })?
        };
        if let Some(found) = found {
            if found.summary.runtime_id != key.runtime_id
                || found.summary.request_id != key.request_id
            {
                return Err(EvalError::new(
                    "HARNESS_CONTRACT_ERROR",
                    "M03 evidence identity does not match the exact MCP correlation key",
                ));
            }
            if found.summary.terminal_state != HistoryTerminalState::Open {
                return Ok(found);
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(EvalError::new(
                "HARNESS_CONTRACT_ERROR",
                format!(
                    "M03 evidence missing for runtime {} request {}",
                    key.runtime_id, key.request_id
                ),
            ));
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_and_strips_exact_correlation_metadata() {
        let mut result = json!({
            "content": [],
            "isError": false,
            "structuredContent": {"code": "OK"},
            "_meta": {
                "chatgptHarness": {
                    "runtimeId": "runtime-1",
                    "requestId": "request-1"
                }
            }
        });
        assert_eq!(
            extract(&result),
            Some(CorrelationKey {
                runtime_id: "runtime-1".into(),
                request_id: "request-1".into(),
            })
        );
        strip_model_metadata(&mut result);
        assert!(result.get("_meta").is_none());
        assert_eq!(result["structuredContent"]["code"], "OK");
    }

    #[test]
    fn missing_metadata_is_not_guessed() {
        assert!(extract(&json!({"isError": false})).is_none());
        assert!(extract(&json!({"_meta": {}})).is_none());
        assert!(extract(&json!({"_meta": {"chatgptHarness": {"runtimeId": "r"}}})).is_none());
    }
}
