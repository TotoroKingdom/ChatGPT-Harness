use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "page", rename_all = "camelCase")]
pub enum HostNavigationIntent {
    Overview,
    Approvals {
        #[serde(rename = "objectId")]
        object_id: String,
    },
    ApprovalsUnavailable {
        #[serde(rename = "objectId")]
        object_id: String,
    },
}

pub fn show_main_window(
    app: &AppHandle,
    intent: Option<HostNavigationIntent>,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is unavailable.".to_owned())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    if let Some(intent) = intent {
        app.emit("desktop:navigation", intent)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn navigation_payload_is_typed_and_bounded() {
        let payload = serde_json::to_value(HostNavigationIntent::Approvals {
            object_id: "request-1".into(),
        })
        .unwrap();
        assert_eq!(payload["page"], "approvals");
        assert_eq!(payload["objectId"], "request-1");
        assert!(payload.get("rawArguments").is_none());
    }
}
