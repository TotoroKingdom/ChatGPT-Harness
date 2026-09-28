use super::*;
use crate::policy::PolicyEngine;
use crate::tools::{EffectiveToolCatalog, ReadFileRequest, ToolRegistry};
use harness_control_protocol::{
    ActivityEventType, DecimalU64, PolicyRule, PolicySource, RuntimeConfigSnapshot,
    RuntimeErrorSummary, RuntimeLimits, SecurityConfig, SecurityMode, WorkspaceConfig,
};
use serde_json::json;
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("harness-diagnostics-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("fixture workspace");
        Self(root)
    }
    fn config(&self) -> RuntimeConfigSnapshot {
        RuntimeConfigSnapshot {
            config_revision: DecimalU64::new(7),
            workspace: WorkspaceConfig {
                root_path: self.0.to_string_lossy().into_owned(),
            },
            security: SecurityConfig {
                mode: SecurityMode::Normal,
                developer_test_mode: false,
            },
            runtime_limits: RuntimeLimits {
                max_concurrent_requests: 4,
                max_activity_events: 20,
                max_read_bytes: 4096,
            },
        }
    }
    fn ready_core(&self) -> RuntimeCore {
        let mut core = RuntimeCore::new();
        core.initialize(self.config()).expect("initialize");
        core.acknowledge().expect("acknowledge");
        core
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove owned fixture");
    }
}

#[test]
fn snapshot_exists_before_runtime_state_and_does_not_claim_ready() {
    let core = RuntimeCore::new();
    assert!(core.status().is_none());
    let snapshot = core.diagnostics();
    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.runtime.readiness, RuntimeReadiness::NotReady);
    assert_eq!(
        snapshot.runtime.initialized_modules,
        vec![
            InitializedModule::ToolRegistry,
            InitializedModule::PolicyEngine
        ]
    );
    assert_eq!(
        snapshot.security.workspace_validation,
        WorkspaceValidation::NotInitialized
    );
    assert!(snapshot.security.policy_engine_loaded);
    assert!(!snapshot.security.audit_enabled);
    assert_eq!(
        snapshot.security.execution_pipeline,
        PipelineState::NotInitialized
    );
    assert_eq!(snapshot.tools.declared_count, 4);
    assert_eq!(snapshot.tools.effective_count, 0);
    assert!(snapshot
        .tools
        .items
        .iter()
        .all(|tool| tool.availability == DiagnosticToolAvailability::UnavailableByRuntime));
}

#[test]
fn initialization_ack_stop_and_rollback_follow_owned_lifecycle() {
    let fixture = Fixture::new();
    let mut core = RuntimeCore::new();
    core.initialize(fixture.config()).expect("initialize");
    let initialized = core.diagnostics();
    assert_eq!(
        initialized.runtime.readiness,
        RuntimeReadiness::Initializing
    );
    assert_eq!(
        initialized.security.execution_pipeline,
        PipelineState::NotReady
    );
    assert_eq!(
        initialized.security.workspace_validation,
        WorkspaceValidation::ValidatedAtInitialization
    );
    assert_eq!(initialized.runtime.initialized_modules.len(), 6);
    assert!(initialized.security.audit_enabled);
    assert_eq!(initialized.tools.effective_count, 0);

    core.acknowledge().expect("acknowledge");
    let ready = core.diagnostics();
    assert_eq!(ready.runtime.readiness, RuntimeReadiness::Ready);
    assert_eq!(ready.security.execution_pipeline, PipelineState::Ready);
    assert_eq!(ready.tools.effective_count, 4);
    core.stop().expect("stop");
    let stopped = core.diagnostics();
    assert_eq!(stopped.runtime.lifecycle, RuntimeLifecycle::Stopping);
    assert_eq!(stopped.runtime.readiness, RuntimeReadiness::NotReady);
    assert_eq!(stopped.security.execution_pipeline, PipelineState::NotReady);
    assert_eq!(stopped.tools.effective_count, 0);
    assert_eq!(stopped.tunnel, ready.tunnel);

    core.rollback_initialize();
    let rolled_back = core.diagnostics();
    assert_eq!(rolled_back.runtime.runtime_id, ready.runtime.runtime_id);
    assert_eq!(rolled_back.runtime.started_at, ready.runtime.started_at);
    assert_eq!(
        rolled_back.security.workspace_validation,
        WorkspaceValidation::NotInitialized
    );
    assert_eq!(
        rolled_back.security.execution_pipeline,
        PipelineState::NotInitialized
    );
    assert!(!rolled_back.security.audit_enabled);
}

#[test]
fn failed_initialization_cannot_be_presented_as_validated() {
    let fixture = Fixture::new();
    let mut config = fixture.config();
    config.workspace.root_path = fixture.0.join("missing").to_string_lossy().into_owned();
    let mut core = RuntimeCore::new();
    assert!(core.initialize(config).is_err());
    let snapshot = core.diagnostics();
    assert_eq!(
        snapshot.security.workspace_validation,
        WorkspaceValidation::NotInitialized
    );
    assert_eq!(snapshot.tools.effective_count, 0);
    // RuntimeCore does not retain initialization error history. Do not fabricate it.
    assert_eq!(
        snapshot.last_error.observation,
        ErrorObservation::NotRecorded
    );
}

#[test]
fn reads_are_side_effect_free_and_uninstrumented_requests_are_never_zero() {
    let fixture = Fixture::new();
    let core = fixture.ready_core();
    let before = serde_json::to_value(core.status()).unwrap();
    let sequence = core.events.as_ref().unwrap().latest_sequence();
    let tools = core.effective_tools();
    let mut first = core.diagnostics();
    let mut second = core.diagnostics();
    assert!(chrono::DateTime::parse_from_rfc3339(&first.captured_at).is_ok());
    first.captured_at.clear();
    second.captured_at.clear();
    assert_eq!(first, second);
    assert_eq!(serde_json::to_value(core.status()).unwrap(), before);
    assert_eq!(core.events.as_ref().unwrap().latest_sequence(), sequence);
    assert_eq!(core.effective_tools(), tools);
    assert_eq!(
        first.requests.observation,
        RequestObservation::NotInstrumented
    );
    assert_eq!(first.requests.active_count, None);
    fs::write(fixture.0.join("file.txt"), "test").unwrap();
    core.read_file(&ReadFileRequest::new("file.txt")).unwrap();
    assert_eq!(core.diagnostics().requests.active_count, None);
}

#[test]
fn tool_statuses_are_exact_projections_of_the_existing_effective_catalog() {
    let policy = |decision| {
        PolicyEngine::new(vec![PolicyRule {
            id: "test".into(),
            name: "test".into(),
            decision,
            scope: "active_workspace".into(),
            reason: "test".into(),
            source: PolicySource::User,
            editable: false,
            enabled: true,
        }])
    };
    for (registry, engine, ready, expected) in [
        (
            ToolRegistry::normal_mode(),
            policy(PolicyDecision::Allow),
            true,
            DiagnosticToolAvailability::Available,
        ),
        (
            ToolRegistry::normal_mode(),
            policy(PolicyDecision::Deny),
            true,
            DiagnosticToolAvailability::UnavailableByPolicy,
        ),
        (
            ToolRegistry::normal_mode(),
            policy(PolicyDecision::Ask),
            true,
            DiagnosticToolAvailability::UnavailableByPolicy,
        ),
        (
            ToolRegistry::normal_mode(),
            policy(PolicyDecision::Allow),
            false,
            DiagnosticToolAvailability::UnavailableByRuntime,
        ),
        (
            ToolRegistry::normal_mode().with_test_enabled(0, false),
            policy(PolicyDecision::Allow),
            true,
            DiagnosticToolAvailability::Disabled,
        ),
        (
            ToolRegistry::normal_mode(),
            PolicyEngine::new(vec![]),
            true,
            DiagnosticToolAvailability::UnavailableByPolicy,
        ),
    ] {
        let effective =
            EffectiveToolCatalog::compute(&registry, &engine, SecurityMode::Normal, ready);
        let projected = ToolDiagnostic::from(effective[0].clone());
        assert_eq!(projected.availability, expected);
        assert_eq!(projected.decision, effective[0].decision);
        assert_eq!(projected.name, effective[0].name);
        assert_eq!(
            projected.declared_capability,
            effective[0].declared_capability
        );
    }
    for (state, serialized) in [
        (DiagnosticToolAvailability::Available, "available"),
        (
            DiagnosticToolAvailability::UnavailableByPolicy,
            "unavailable_by_policy",
        ),
        (
            DiagnosticToolAvailability::UnavailableByRuntime,
            "unavailable_by_runtime",
        ),
        (DiagnosticToolAvailability::Disabled, "disabled"),
    ] {
        assert_eq!(serde_json::to_value(state).unwrap(), serialized);
    }
}

#[test]
fn serialized_and_debug_exports_drop_sensitive_sources() {
    let fixture = Fixture::new();
    let mut core = fixture.ready_core();
    let sensitive = r"API_KEY=sentinel-secret; Token=sentinel-token; credential=sentinel-credential; C:\private\sensitive; /private/sensitive";
    core.last_error = Some(RuntimeErrorSummary {
        code: sensitive.into(),
        message: sensitive.into(),
    });
    core.publish(
        ActivityEventType::RuntimeStarted,
        sensitive,
        [("secret".into(), sensitive.into())],
        Some(sensitive.into()),
    );
    core.runtime_id = sensitive.into();
    core.started_at = sensitive.into();
    let snapshot = core.diagnostics();
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(value["runtime"]["runtimeId"], json!(null));
    assert_eq!(value["runtime"]["startedAt"], json!(null));
    assert_eq!(
        value["lastError"],
        json!({"observation":"recorded", "code":null})
    );
    let text = format!("{} {:?}", value, snapshot);
    for excluded in [
        sensitive,
        fixture.0.to_str().unwrap(),
        "sentinel-secret",
        "sentinel-token",
        "sentinel-credential",
        "sensitive",
        "activeWorkspace",
        "rootPath",
        "metadata",
    ] {
        assert!(
            !text.contains(excluded),
            "unsafe projection retained {excluded}"
        );
    }
    let mut tool = core.effective_tools().remove(0);
    tool.description = sensitive.into();
    tool.reason = sensitive.into();
    assert!(!serde_json::to_string(&ToolDiagnostic::from(tool))
        .unwrap()
        .contains("sentinel"));
}

#[test]
fn recorded_known_error_preserves_only_an_allowlisted_code() {
    let mut core = RuntimeCore::new();
    core.last_error = Some(RuntimeErrorSummary {
        code: "RUNTIME_NOT_READY".into(),
        message: "sensitive free text".into(),
    });
    assert_eq!(
        serde_json::to_value(core.diagnostics().last_error).unwrap(),
        json!({"observation":"recorded", "code":"RUNTIME_NOT_READY"})
    );
}

#[test]
fn v1_json_shape_is_frozen_and_local_readiness_never_establishes_tunnel_readiness() {
    let fixture = Fixture::new();
    let core = fixture.ready_core();
    let snapshot = core.diagnostics();
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        value,
        json!({
            "schemaVersion": 1,
            "capturedAt": snapshot.captured_at,
            "runtime": {
                "runtimeId": core.runtime_id,
                "startedAt": core.started_at,
                "lifecycle": "ready",
                "readiness": "ready",
                "initializedModules": ["tool_registry", "policy_engine", "workspace_guard", "execution_pipeline", "audit_emitter", "event_bus"]
            },
            "security": {
                "workspaceValidation": "validated_at_initialization",
                "policyEngineLoaded": true,
                "auditEnabled": true,
                "executionPipeline": "ready"
            },
            "tools": {
                "declaredCount": 4,
                "effectiveCount": 4,
                "items": [
                    {"name":"read_file", "declaredCapability":"workspace_read", "availability":"available", "decision":"allow"},
                    {"name":"glob", "declaredCapability":"workspace_read", "availability":"available", "decision":"allow"},
                    {"name":"write", "declaredCapability":"workspace_write", "availability":"available", "decision":"allow"},
                    {"name":"edit", "declaredCapability":"workspace_write", "availability":"available", "decision":"allow"}
                ]
            },
            "requests": {"observation":"not_instrumented", "activeCount":null},
            "lastError": {"observation":"not_recorded", "code":null},
            "tunnel": {"configured":null, "processState":"not_observed", "connectionState":"not_established", "blockedReason":"m7_c_blocked"}
        })
    );
}
