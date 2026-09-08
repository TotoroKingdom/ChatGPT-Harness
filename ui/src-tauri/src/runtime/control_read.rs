//! Typed local reads; never forward transport/parser error text to the UI.
use super::{RuntimeSupervisor, SupervisorError};
use harness_control_protocol::control_read::{
    ActivityEntry, ActivitySnapshotParams, ReadErrorCode, RuntimeDiagnosticsSnapshot,
    ToolDiagnostic, ACTIVITY_SNAPSHOT, DIAGNOSTICS_GET, EFFECTIVE_TOOLS_GET,
};
use serde_json::json;

impl RuntimeSupervisor {
    pub fn get_diagnostics(&self) -> Result<RuntimeDiagnosticsSnapshot, ReadErrorCode> {
        let snapshot: RuntimeDiagnosticsSnapshot = self
            .request(DIAGNOSTICS_GET, json!({}))
            .map_err(safe_error)?;
        if snapshot.schema_version != 1 {
            return Err(ReadErrorCode::InternalError);
        }
        Ok(snapshot)
    }

    // Value accepts malformed/missing caller input here so validation errors use
    // the same ControlError instead of Tauri's unstructured argument error text.
    pub fn get_activity_snapshot(
        &self,
        limit: serde_json::Value,
    ) -> Result<Vec<ActivityEntry>, ReadErrorCode> {
        let limit = limit.as_u64().ok_or(ReadErrorCode::InvalidRequest)?;
        let params = ActivitySnapshotParams { limit };
        params.validate()?;
        let items: Vec<ActivityEntry> = self
            .request_serialized(ACTIVITY_SNAPSHOT, params)
            .map_err(safe_error)?;
        if items.len() > limit as usize {
            return Err(ReadErrorCode::InternalError);
        }
        Ok(items)
    }

    pub fn get_effective_tools(&self) -> Result<Vec<ToolDiagnostic>, ReadErrorCode> {
        self.request(EFFECTIVE_TOOLS_GET, json!({}))
            .map_err(safe_error)
    }
}

fn safe_error(error: SupervisorError) -> ReadErrorCode {
    match error.code.as_str() {
        "RUNTIME_DISCONNECTED" | "RUNTIME_CRASHED" | "RUNTIME_TIMEOUT" => {
            ReadErrorCode::RuntimeUnavailable
        }
        "RPC_RUNTIME_NOT_READY" | "RPC_NOT_INITIALIZED" | "LIFECYCLE_CONFLICT" => {
            ReadErrorCode::RuntimeNotReady
        }
        "RPC_INVALID_ARGUMENT" => ReadErrorCode::InvalidRequest,
        _ => ReadErrorCode::InternalError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unavailable_is_not_an_empty_or_ready_snapshot() {
        let supervisor = RuntimeSupervisor::new();
        assert_eq!(
            supervisor.get_diagnostics().unwrap_err(),
            ReadErrorCode::RuntimeUnavailable
        );
        assert_eq!(
            supervisor.get_activity_snapshot(json!(1)).unwrap_err(),
            ReadErrorCode::RuntimeUnavailable
        );
        assert_eq!(
            supervisor.get_effective_tools().unwrap_err(),
            ReadErrorCode::RuntimeUnavailable
        );
    }
    #[test]
    fn validates_limit_before_attempting_transport() {
        let supervisor = RuntimeSupervisor::new();
        for limit in [
            json!(0),
            json!(257),
            json!(u64::MAX),
            json!(-1),
            json!(1.5),
            json!("secret C:/private"),
            json!(null),
            json!(true),
            json!({}),
        ] {
            assert_eq!(
                supervisor.get_activity_snapshot(limit).unwrap_err(),
                ReadErrorCode::InvalidRequest
            );
        }
    }
    #[test]
    fn errors_are_closed_and_never_copy_transport_text() {
        for (code, expected) in [
            ("RPC_RUNTIME_NOT_READY", ReadErrorCode::RuntimeNotReady),
            ("RPC_NOT_INITIALIZED", ReadErrorCode::RuntimeNotReady),
            ("RPC_INVALID_ARGUMENT", ReadErrorCode::InvalidRequest),
            ("RUNTIME_TIMEOUT", ReadErrorCode::RuntimeUnavailable),
            ("RUNTIME_INVALID_RESPONSE", ReadErrorCode::InternalError),
            ("untrusted-token", ReadErrorCode::InternalError),
        ] {
            let mapped = safe_error(SupervisorError {
                code: code.into(),
                message: "api-key secret token credentials raw arguments C:/private".into(),
            });
            assert_eq!(mapped, expected);
            let error = crate::commands::ControlError::from(mapped);
            assert_eq!(
                serde_json::to_value(error).unwrap(),
                json!({ "code": expected.as_str(), "message": expected.message() })
            );
        }
    }
}
