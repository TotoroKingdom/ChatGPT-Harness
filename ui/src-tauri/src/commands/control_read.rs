//! M8-C command adapters only; no configuration, secret store, or lifecycle work.
use super::ControlError;
use crate::runtime::RuntimeSupervisor;
use harness_control_protocol::control_read::{
    ActivityEntry, ReadErrorCode, RuntimeDiagnosticsSnapshot, ToolDiagnostic,
};
use tauri::State;

impl From<ReadErrorCode> for ControlError {
    fn from(code: ReadErrorCode) -> Self {
        Self {
            code: code.as_str().into(),
            message: code.message().into(),
        }
    }
}

#[tauri::command]
pub fn runtime_get_diagnostics(
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<RuntimeDiagnosticsSnapshot, ControlError> {
    runtime.get_diagnostics().map_err(Into::into)
}

#[tauri::command]
pub fn runtime_get_activity_snapshot(
    runtime: State<'_, RuntimeSupervisor>,
    limit: Option<serde_json::Value>,
) -> Result<Vec<ActivityEntry>, ControlError> {
    runtime
        .get_activity_snapshot(limit.unwrap_or(serde_json::Value::Null))
        .map_err(Into::into)
}

#[tauri::command]
pub fn runtime_get_effective_tools(
    runtime: State<'_, RuntimeSupervisor>,
) -> Result<Vec<ToolDiagnostic>, ControlError> {
    runtime.get_effective_tools().map_err(Into::into)
}
