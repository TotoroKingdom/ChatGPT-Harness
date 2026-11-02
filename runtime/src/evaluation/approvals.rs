use super::mcp_client::EvalRuntimeHarness;
use super::{EvalError, EvalResult};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) struct PendingApproval {
    pub(crate) request_id: String,
    pub(crate) runtime_id: String,
    pub(crate) tool: String,
    pub(crate) sanitized_arguments: BTreeMap<String, String>,
}

impl PendingApproval {
    fn from_value(value: &Value) -> EvalResult<Self> {
        Ok(Self {
            request_id: value
                .get("requestId")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvalError::new(
                        "HARNESS_CONTRACT_ERROR",
                        "pending approval omitted requestId",
                    )
                })?
                .to_owned(),
            runtime_id: value
                .get("runtimeId")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvalError::new(
                        "HARNESS_CONTRACT_ERROR",
                        "pending approval omitted runtimeId",
                    )
                })?
                .to_owned(),
            tool: value
                .get("tool")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    EvalError::new("HARNESS_CONTRACT_ERROR", "pending approval omitted tool")
                })?
                .to_owned(),
            sanitized_arguments: value
                .get("sanitizedArguments")
                .cloned()
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_default(),
        })
    }
}

pub(crate) async fn list_pending(
    harness: &mut EvalRuntimeHarness,
) -> EvalResult<Vec<PendingApproval>> {
    let value = harness
        .control_request("approvals.list", json!({ "status": "pending" }))
        .await?;
    let items = value
        .get("items")
        .and_then(Value::as_array)
        .ok_or_else(|| EvalError::new("HARNESS_CONTRACT_ERROR", "approvals.list omitted items"))?;
    items.iter().map(PendingApproval::from_value).collect()
}

pub(crate) async fn resolve(
    harness: &mut EvalRuntimeHarness,
    request_id: &str,
    decision: &str,
) -> EvalResult<()> {
    let value = harness
        .control_request(
            "approvals.resolve",
            json!({ "requestId": request_id, "decision": decision }),
        )
        .await?;
    let resolved = value
        .get("approval")
        .and_then(|approval| approval.get("status"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let expected = if decision == "deny" {
        "denied"
    } else {
        "approved"
    };
    if resolved != expected {
        return Err(EvalError::new(
            "HARNESS_CONTRACT_ERROR",
            format!("approval {request_id} terminal status was '{resolved}'"),
        ));
    }
    Ok(())
}
