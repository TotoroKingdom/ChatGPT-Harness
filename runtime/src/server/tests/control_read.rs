use super::*;
use crate::tools::ReadFileRequest;
use harness_control_protocol::control_read as wire;

fn call(core: &mut RuntimeCore, method: &str, params: Value) -> RpcMessage {
    let mut service = McpRuntimeService::new();
    let mut output = Vec::new();
    let phase = core.phase;
    assert!(!handle_request(
        RpcRequest {
            id: harness_control_protocol::RequestId::String("read-test".into()),
            method: method.into(),
            params: Some(params),
        },
        core,
        &mut service,
        &mut false,
        &mut output
    )
    .unwrap());
    assert_eq!(core.phase, phase, "read must not advance lifecycle");
    serde_json::from_slice(&output).unwrap()
}
fn result(message: RpcMessage) -> Value {
    match message {
        RpcMessage::Response(response) => response.result,
        other => panic!("expected result, got {other:?}"),
    }
}
fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned>(value: Value) {
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
}

#[test]
fn read_apis_preserve_initialization_and_ack_gates() {
    let mut core = RuntimeCore::new();
    for method in [
        wire::DIAGNOSTICS_GET,
        wire::ACTIVITY_SNAPSHOT,
        wire::EFFECTIVE_TOOLS_GET,
        "tools.list",
    ] {
        let params = if method == wire::ACTIVITY_SNAPSHOT {
            json!({"limit":1})
        } else {
            json!({})
        };
        let RpcMessage::Error(error) = call(&mut core, method, params) else {
            panic!("initialization required")
        };
        assert_eq!(error.error.data.code, MachineErrorCode::NotInitialized);
    }
    let workspace = TestWorkspace::new();
    core.initialize(runtime_config(&workspace.root)).unwrap();
    for phase in [Phase::InitializedPendingAck, Phase::Stopping] {
        core.phase = phase;
        for method in [
            wire::DIAGNOSTICS_GET,
            wire::ACTIVITY_SNAPSHOT,
            wire::EFFECTIVE_TOOLS_GET,
        ] {
            let params = if method == wire::ACTIVITY_SNAPSHOT {
                json!({"limit":1})
            } else {
                json!({})
            };
            let RpcMessage::Error(error) = call(&mut core, method, params) else {
                panic!("not ready")
            };
            assert_eq!(error.error.data.code, MachineErrorCode::RuntimeNotReady);
        }
    }
}

#[test]
fn all_reads_use_existing_safe_snapshots_and_roundtrip_launcher_contracts() {
    let workspace = TestWorkspace::new();
    fs::write(workspace.root.join("PRIVATE-ARG.txt"), "RAW-RESULT-SECRET").unwrap();
    let mut core = RuntimeCore::new();
    core.initialize(runtime_config(&workspace.root)).unwrap();
    core.acknowledge().unwrap();
    assert_eq!(
        result(call(
            &mut core,
            wire::ACTIVITY_SNAPSHOT,
            json!({"limit":256})
        )),
        json!([])
    );
    core.read_file(&ReadFileRequest::new("PRIVATE-ARG.txt"))
        .unwrap();
    core.last_error = Some(harness_control_protocol::RuntimeErrorSummary {
        code: "UNTRUSTED-TOKEN".into(),
        message: "API-KEY credentials secret C:/private".into(),
    });
    let original = core.activity_snapshot(256);
    assert!(!original.is_empty());
    let activity = result(call(
        &mut core,
        wire::ACTIVITY_SNAPSHOT,
        json!({"limit":256}),
    ));
    assert_eq!(activity, serde_json::to_value(&original).unwrap());
    roundtrip::<Vec<wire::ActivityEntry>>(activity.clone());
    let tail = result(call(&mut core, wire::ACTIVITY_SNAPSHOT, json!({"limit":1})));
    assert_eq!(
        tail,
        serde_json::to_value(core.activity_snapshot(1)).unwrap()
    );
    assert_eq!(
        tail[0]["eventId"],
        activity.as_array().unwrap().last().unwrap()["eventId"]
    );
    let tools = result(call(&mut core, wire::EFFECTIVE_TOOLS_GET, json!({})));
    assert_eq!(
        tools,
        serde_json::to_value(core.diagnostics().tools.items).unwrap()
    );
    roundtrip::<Vec<wire::ToolDiagnostic>>(tools.clone());
    assert_eq!(tools.as_array().unwrap().len(), 4);
    assert_eq!(tools[0]["availability"], "available");
    assert_eq!(tools[0]["decision"], "allow");
    let diagnostics = result(call(&mut core, wire::DIAGNOSTICS_GET, json!({})));
    roundtrip::<wire::RuntimeDiagnosticsSnapshot>(diagnostics.clone());
    assert_eq!(diagnostics["runtime"]["readiness"], "ready");
    assert_eq!(diagnostics["tunnel"]["connectionState"], "not_established");
    assert_eq!(diagnostics["tunnel"]["blockedReason"], "m7_c_blocked");
    assert_eq!(diagnostics["lastError"]["code"], Value::Null);
    for value in [&activity, &tools, &diagnostics] {
        let serialized = serde_json::to_string(value).unwrap();
        for forbidden in [
            "PRIVATE-ARG",
            "RAW-RESULT",
            "UNTRUSTED-TOKEN",
            "API-KEY",
            "credentials",
            "C:/private",
            workspace.root.to_str().unwrap(),
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
    }
    assert_eq!(
        core.activity_snapshot(256),
        original,
        "reads must not emit or consume activity"
    );
    assert_eq!(
        result(call(
            &mut core,
            wire::ACTIVITY_SNAPSHOT,
            json!({"limit":256})
        )),
        activity
    );
    // A widened/contaminated response fails closed instead of transparently
    // passing new raw data through the Launcher.
    let mut polluted = diagnostics;
    polluted["runtime"]["credentials"] = json!("secret");
    assert!(serde_json::from_value::<wire::RuntimeDiagnosticsSnapshot>(polluted).is_err());
    let mut polluted = tools;
    polluted[0]["reason"] = json!("C:/private");
    assert!(serde_json::from_value::<Vec<wire::ToolDiagnostic>>(polluted).is_err());
}

#[test]
fn invalid_limits_and_extra_parameters_are_rejected_without_echo() {
    let workspace = TestWorkspace::new();
    let mut core = RuntimeCore::new();
    core.initialize(runtime_config(&workspace.root)).unwrap();
    core.acknowledge().unwrap();
    for params in [
        json!({}),
        json!({"limit":0}),
        json!({"limit":257}),
        json!({"limit":u64::MAX}),
        json!({"limit":-1}),
        json!({"limit":1.5}),
        json!({"limit":"SECRET"}),
        json!({"limit":null}),
        json!({"limit":true}),
        json!({"limit":1,"credentials":"SECRET"}),
        json!(null),
        json!([]),
    ] {
        let RpcMessage::Error(error) = call(&mut core, wire::ACTIVITY_SNAPSHOT, params) else {
            panic!("invalid request accepted")
        };
        assert_eq!(error.error.code, error_codes::INVALID_PARAMS);
        assert_eq!(error.error.data.code, MachineErrorCode::InvalidArgument);
        assert_eq!(
            error.error.message,
            wire::ReadErrorCode::InvalidRequest.message()
        );
        assert!(error.error.data.details.is_none());
    }
    for method in [wire::DIAGNOSTICS_GET, wire::EFFECTIVE_TOOLS_GET] {
        let RpcMessage::Error(error) = call(&mut core, method, json!({"token":"SECRET"})) else {
            panic!("unknown field")
        };
        assert_eq!(error.error.data.code, MachineErrorCode::InvalidArgument);
        assert!(!serde_json::to_string(&error).unwrap().contains("SECRET"));
    }
    assert_eq!(
        wire::MAX_ACTIVITY_LIMIT as usize,
        crate::activity::ACTIVITY_CAPACITY
    );
}

#[test]
fn read_methods_work_on_the_real_stdio_dispatch_loop() {
    let workspace = TestWorkspace::new();
    let messages = run_lines(&[
        request(
            "init",
            "initialize",
            initialize_params(PROTOCOL_VERSION, &workspace.root),
        ),
        notification("initialized"),
        request("diag", wire::DIAGNOSTICS_GET, json!({})),
        request("activity", wire::ACTIVITY_SNAPSHOT, json!({"limit":256})),
        request("tools", wire::EFFECTIVE_TOOLS_GET, json!({})),
        request("stop", "runtime.stop", json!({})),
    ]);
    assert_eq!(messages.len(), 5);
    assert!(messages
        .iter()
        .all(|m| matches!(m, RpcMessage::Response(_))));
    roundtrip::<wire::RuntimeDiagnosticsSnapshot>(result(messages[1].clone()));
    roundtrip::<Vec<wire::ActivityEntry>>(result(messages[2].clone()));
    roundtrip::<Vec<wire::ToolDiagnostic>>(result(messages[3].clone()));
}
