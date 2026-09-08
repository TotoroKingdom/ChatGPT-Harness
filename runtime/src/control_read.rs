//! Read-only control dispatch over existing safe Runtime projections.
use crate::diagnostics::ToolDiagnostic;
use crate::state::{Phase, RuntimeCore};
use harness_control_protocol::control_read::{
    ActivitySnapshotParams, ReadErrorCode, ReadParams, ACTIVITY_SNAPSHOT, DIAGNOSTICS_GET,
    EFFECTIVE_TOOLS_GET,
};
use harness_control_protocol::RuntimeReadiness;
use serde::Serialize;
use serde_json::Value;

pub(crate) fn handles(method: &str) -> bool {
    matches!(
        method,
        DIAGNOSTICS_GET | ACTIVITY_SNAPSHOT | EFFECTIVE_TOOLS_GET
    )
}

pub(crate) fn read(
    core: &RuntimeCore,
    method: &str,
    params: Option<Value>,
) -> Result<Value, ReadErrorCode> {
    let params = params.unwrap_or_else(|| serde_json::json!({}));
    let limit = if method == ACTIVITY_SNAPSHOT {
        let params: ActivitySnapshotParams =
            serde_json::from_value(params).map_err(|_| ReadErrorCode::InvalidRequest)?;
        Some(params.validate()?)
    } else {
        let _: ReadParams =
            serde_json::from_value(params).map_err(|_| ReadErrorCode::InvalidRequest)?;
        None
    };
    // The existing IPC handshake gate runs before this read-only adapter.
    // Keep diagnostics observable if the initialized Core loses local readiness;
    // activity/catalog reads must not imply that unavailable data is Empty.
    if core.phase != Phase::Ready
        || (method != DIAGNOSTICS_GET
            && core.diagnostics().runtime.readiness != RuntimeReadiness::Ready)
    {
        return Err(ReadErrorCode::RuntimeNotReady);
    }
    match method {
        DIAGNOSTICS_GET => encode(core.diagnostics()),
        ACTIVITY_SNAPSHOT => {
            encode(core.activity_snapshot(limit.ok_or(ReadErrorCode::InvalidRequest)?))
        }
        EFFECTIVE_TOOLS_GET => encode(
            core.effective_tools()
                .into_iter()
                .map(ToolDiagnostic::from)
                .collect::<Vec<_>>(),
        ),
        _ => Err(ReadErrorCode::InvalidRequest),
    }
}

fn encode(value: impl Serialize) -> Result<Value, ReadErrorCode> {
    serde_json::to_value(value).map_err(|_| ReadErrorCode::InternalError)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct FailsSerialization;
    impl Serialize for FailsSerialization {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("secret token C:/private"))
        }
    }
    #[test]
    fn serialization_failure_is_a_safe_internal_error() {
        assert_eq!(
            encode(FailsSerialization).unwrap_err(),
            ReadErrorCode::InternalError
        );
    }
}
