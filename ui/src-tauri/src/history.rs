use crate::{
    commands::ControlError,
    config::ConfigStore,
    runtime::{RuntimeProcessPhase, RuntimeSupervisor, SupervisorError},
};
use chatgpt_harness_evidence_store::{
    clear_history_offline, status_for_error, EvidenceReader, StoreError,
};
use harness_control_protocol::{
    AppConfig, EvidenceErrorCode, EvidenceStoreState, EvidenceStoreStatus, HistoryClearResult,
    HistoryGetParams, HistoryGetResult, HistoryQueryParams, HistoryQueryResult, MetricsQueryParams,
    MetricsQueryResult, TraceGetParams, TraceGetResult,
};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

pub(crate) fn database_path(app: &AppHandle) -> Result<PathBuf, ControlError> {
    app.path()
        .app_data_dir()
        .map(|directory| directory.join("history").join("history.sqlite3"))
        .map_err(|_| ControlError {
            code: "EVIDENCE_IO".into(),
            message: "History storage is unavailable.".into(),
        })
}

/// Startup inspection is read-only. The Runtime writer acquires ownership
/// before migrating v1 evidence; merely opening the app cannot upgrade it.
pub(crate) fn prepare_before_runtime_start(app: &AppHandle) {
    let Ok(database) = database_path(app) else {
        return;
    };
    if !database.is_file() {
        return;
    }
    let _ = EvidenceReader::open(database);
}

fn desired_config(app: &AppHandle) -> Result<AppConfig, ControlError> {
    let directory = app.path().app_data_dir().map_err(|_| ControlError {
        code: "PATH_ERROR".into(),
        message: "Application data is unavailable.".into(),
    })?;
    ConfigStore::new(directory)
        .load()
        .map_err(ControlError::from)?
        .ok_or_else(|| ControlError {
            code: "CONFIG_NOT_FOUND".into(),
            message: "History configuration is unavailable.".into(),
        })
}

fn with_desired_status(mut status: EvidenceStoreStatus, config: &AppConfig) -> EvidenceStoreStatus {
    status.effective_history_config = config.history.clone();
    if !config.history.enabled {
        status.state = EvidenceStoreState::Disabled;
        status.queue_depth = 0;
        status.queue_capacity = 0;
    }
    status
}

fn evidence_error_code(code: EvidenceErrorCode) -> &'static str {
    match code {
        EvidenceErrorCode::EvidenceBusy => "EVIDENCE_BUSY",
        EvidenceErrorCode::EvidenceWriterOwnership => "EVIDENCE_WRITER_OWNERSHIP",
        EvidenceErrorCode::EvidenceIo => "EVIDENCE_IO",
        EvidenceErrorCode::EvidenceFull => "EVIDENCE_FULL",
        EvidenceErrorCode::EvidenceCorrupt => "EVIDENCE_CORRUPT",
        EvidenceErrorCode::EvidenceIncompatibleSchema => "EVIDENCE_INCOMPATIBLE_SCHEMA",
        EvidenceErrorCode::EvidenceMigrationFailed => "EVIDENCE_MIGRATION_FAILED",
        EvidenceErrorCode::EvidenceQueueOverflow => "EVIDENCE_QUEUE_OVERFLOW",
        EvidenceErrorCode::EvidenceCapacityDegraded => "EVIDENCE_CAPACITY_DEGRADED",
        EvidenceErrorCode::EvidenceInvalidCursor => "EVIDENCE_INVALID_CURSOR",
        EvidenceErrorCode::EvidenceNotFound => "EVIDENCE_NOT_FOUND",
    }
}

fn store_error(error: StoreError, config: &AppConfig) -> ControlError {
    if matches!(&error, StoreError::InvalidEvidence(_)) {
        return ControlError {
            code: "INVALID_REQUEST".into(),
            message: "The evidence query parameters are invalid.".into(),
        };
    }
    let status = status_for_error(&error, config.history.clone());
    ControlError {
        code: evidence_error_code(
            status
                .last_error_code
                .unwrap_or(EvidenceErrorCode::EvidenceIo),
        )
        .into(),
        message: match status.state {
            EvidenceStoreState::Busy => "History storage is busy; retry later.".into(),
            EvidenceStoreState::Incompatible => {
                "Stored History uses an incompatible schema and was preserved.".into()
            }
            EvidenceStoreState::Corrupt => {
                "Stored History is corrupt and was preserved for recovery.".into()
            }
            _ => "History storage operation failed.".into(),
        },
    }
}

fn observability_store_error(error: StoreError, config: &AppConfig) -> ControlError {
    match &error {
        StoreError::InvalidEvidence(_) => ControlError {
            code: "INVALID_REQUEST".into(),
            message: "The observability query parameters are invalid.".into(),
        },
        StoreError::NotFound => ControlError {
            code: "EVIDENCE_NOT_FOUND".into(),
            message: "The retained request evidence was not found.".into(),
        },
        StoreError::IncompatibleSchema { found: 1 } => ControlError {
            code: "EVIDENCE_MIGRATION_REQUIRED".into(),
            message: "Metrics and Trace require the evidence store migration.".into(),
        },
        _ => store_error(error, config),
    }
}

fn observability_disabled() -> ControlError {
    ControlError {
        code: "EVIDENCE_UNAVAILABLE".into(),
        message: "Metrics and Trace are unavailable while History storage is disabled.".into(),
    }
}

pub(crate) fn query(
    app: &AppHandle,
    params: HistoryQueryParams,
) -> Result<HistoryQueryResult, ControlError> {
    let config = desired_config(app)?;
    query_at(database_path(app)?, &config, params)
}

/// Read-only M08 metrics projection. This path never migrates or writes the
/// evidence database and therefore remains safe while Runtime owns the writer.
pub(crate) fn metrics_query(
    app: &AppHandle,
    params: MetricsQueryParams,
) -> Result<MetricsQueryResult, ControlError> {
    let config = desired_config(app)?;
    if !config.history.enabled {
        return Err(observability_disabled());
    }
    metrics_query_at(database_path(app)?, &config, params)
}

fn metrics_query_at(
    database: PathBuf,
    config: &AppConfig,
    params: MetricsQueryParams,
) -> Result<MetricsQueryResult, ControlError> {
    if !config.history.enabled {
        return Err(observability_disabled());
    }
    let reader =
        EvidenceReader::open(database).map_err(|error| observability_store_error(error, config))?;
    let mut result = reader
        .metrics_query(&params)
        .map_err(|error| observability_store_error(error, config))?;
    result.coverage.store_status = with_desired_status(result.coverage.store_status, config);
    Ok(result)
}

/// Read-only exact request trace projection. Correlation is always the
/// canonical `(runtime_id, request_id)` pair; no timestamp fallback exists.
pub(crate) fn trace_get(
    app: &AppHandle,
    params: TraceGetParams,
) -> Result<TraceGetResult, ControlError> {
    let config = desired_config(app)?;
    if !config.history.enabled {
        return Err(observability_disabled());
    }
    trace_get_at(database_path(app)?, &config, params)
}

fn trace_get_at(
    database: PathBuf,
    config: &AppConfig,
    params: TraceGetParams,
) -> Result<TraceGetResult, ControlError> {
    if !config.history.enabled {
        return Err(observability_disabled());
    }
    let reader =
        EvidenceReader::open(database).map_err(|error| observability_store_error(error, config))?;
    let mut result = reader
        .trace_get(&params)
        .map_err(|error| observability_store_error(error, config))?;
    result.store_status = with_desired_status(result.store_status, config);
    Ok(result)
}

#[cfg(test)]
pub(crate) fn metrics_query_at_for_test(
    database: PathBuf,
    config: &AppConfig,
    params: MetricsQueryParams,
) -> Result<MetricsQueryResult, ControlError> {
    metrics_query_at(database, config, params)
}

#[cfg(test)]
pub(crate) fn trace_get_at_for_test(
    database: PathBuf,
    config: &AppConfig,
    params: TraceGetParams,
) -> Result<TraceGetResult, ControlError> {
    trace_get_at(database, config, params)
}

fn query_at(
    database: PathBuf,
    config: &AppConfig,
    params: HistoryQueryParams,
) -> Result<HistoryQueryResult, ControlError> {
    let reader = EvidenceReader::open(database).map_err(|error| store_error(error, config))?;
    let mut result = reader
        .query(&params)
        .map_err(|error| store_error(error, config))?;
    result.store_status = with_desired_status(result.store_status, config);
    Ok(result)
}

#[cfg(test)]
pub(crate) fn query_at_for_test(
    database: PathBuf,
    config: &AppConfig,
    params: HistoryQueryParams,
) -> Result<HistoryQueryResult, ControlError> {
    query_at(database, config, params)
}

pub(crate) fn get(
    app: &AppHandle,
    params: HistoryGetParams,
) -> Result<HistoryGetResult, ControlError> {
    let config = desired_config(app)?;
    get_at(database_path(app)?, &config, params)
}

fn get_at(
    database: PathBuf,
    config: &AppConfig,
    params: HistoryGetParams,
) -> Result<HistoryGetResult, ControlError> {
    let reader = EvidenceReader::open(database).map_err(|error| store_error(error, config))?;
    reader
        .get(&params.runtime_id, &params.request_id)
        .map_err(|error| store_error(error, config))?
        .map(|mut result| {
            result.store_status = with_desired_status(result.store_status, config);
            result
        })
        .ok_or_else(|| ControlError {
            code: "EVIDENCE_NOT_FOUND".into(),
            message: "History request was not found.".into(),
        })
}

#[cfg(test)]
pub(crate) fn get_at_for_test(
    database: PathBuf,
    config: &AppConfig,
    params: HistoryGetParams,
) -> Result<HistoryGetResult, ControlError> {
    get_at(database, config, params)
}

pub(crate) fn status(
    app: &AppHandle,
    runtime: &RuntimeSupervisor,
) -> Result<EvidenceStoreStatus, ControlError> {
    let config = desired_config(app)?;
    let process = runtime.process_status();
    match process.phase {
        RuntimeProcessPhase::Running => {
            return runtime
                .evidence_status()
                .map(|status| with_desired_status(status, &config))
                .map_err(control_error);
        }
        RuntimeProcessPhase::Starting | RuntimeProcessPhase::Stopping => {
            return Ok(EvidenceStoreStatus {
                state: EvidenceStoreState::Busy,
                schema_version: None,
                queue_depth: 0,
                queue_capacity: 1024,
                drop_count: 0,
                last_drop_at_utc: None,
                last_commit_at_utc: None,
                last_error_code: Some(EvidenceErrorCode::EvidenceBusy),
                effective_history_config: config.history,
                footprint_bytes: None,
                coverage_unknown_since_utc: None,
            });
        }
        RuntimeProcessPhase::Stopped | RuntimeProcessPhase::Crashed => {}
    }
    offline_status_at(database_path(app)?, &config)
}

fn offline_status_at(
    database: PathBuf,
    config: &AppConfig,
) -> Result<EvidenceStoreStatus, ControlError> {
    let status = EvidenceReader::open(database)
        .map_err(|error| store_error(error, config))?
        .status()
        .map_err(|error| store_error(error, config))?;
    Ok(with_desired_status(status, config))
}

pub(crate) fn clear(
    app: &AppHandle,
    runtime: &RuntimeSupervisor,
) -> Result<HistoryClearResult, ControlError> {
    let config = desired_config(app)?;
    match runtime.process_status().phase {
        RuntimeProcessPhase::Running => {
            let status = runtime.evidence_status().map_err(control_error)?;
            if status.state == EvidenceStoreState::Disabled {
                // A disabled Runtime generation owns no writer. Clear still
                // acquires the shared maintenance lock, so it cannot race a
                // normal writer and remains available without a restart.
                return clear_offline_at(database_path(app)?, &config);
            }
            return runtime.history_clear().map_err(control_error);
        }
        RuntimeProcessPhase::Starting
        | RuntimeProcessPhase::Stopping
        | RuntimeProcessPhase::Crashed => {
            return Err(ControlError {
                code: "EVIDENCE_BUSY".into(),
                message: "Runtime lifecycle is changing; History was not cleared.".into(),
            });
        }
        RuntimeProcessPhase::Stopped => {}
    }
    clear_offline_at(database_path(app)?, &config)
}

fn clear_offline_at(
    database: PathBuf,
    config: &AppConfig,
) -> Result<HistoryClearResult, ControlError> {
    clear_history_offline(database).map_err(|error| store_error(error, config))
}

#[cfg(test)]
pub(crate) fn clear_offline_at_for_test(
    database: PathBuf,
    config: &AppConfig,
) -> Result<HistoryClearResult, ControlError> {
    clear_offline_at(database, config)
}

/// Test-only seam for the online Tauri History command.  The real command
/// chooses this branch after reading the live Runtime evidence status; the
/// callback is the real Runtime `history.clear` control request supplied by
/// the integration test.
#[cfg(test)]
pub(crate) fn clear_online_at_for_test<F>(
    database: PathBuf,
    config: &AppConfig,
    status: EvidenceStoreStatus,
    clear_runtime: F,
) -> Result<HistoryClearResult, ControlError>
where
    F: FnOnce() -> Result<HistoryClearResult, ControlError>,
{
    if status.state == EvidenceStoreState::Disabled {
        clear_offline_at(database, config)
    } else {
        clear_runtime()
    }
}

fn control_error(error: SupervisorError) -> ControlError {
    ControlError {
        code: if error.code == "RUNTIME_DISCONNECTED" {
            "EVIDENCE_IO".into()
        } else {
            error.code
        },
        message: error.message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chatgpt_harness_evidence_store::{
        EvidenceCoverage, EvidenceEventV1, EvidenceStage, EvidenceWorkspaceValidation,
        EvidenceWriter, HistoryOutcome, RuntimeSession, WriterOptions,
    };
    use harness_control_protocol::{EvidenceErrorCode, EvidenceStoreState, HistoryConfig};

    fn app_config(history: HistoryConfig) -> AppConfig {
        AppConfig {
            schema_version: 3,
            config_revision: harness_control_protocol::DecimalU64::new(7),
            workspace: harness_control_protocol::WorkspaceConfig {
                root_path: "C:\\workspace".into(),
            },
            tunnel: harness_control_protocol::TunnelConfig {
                tunnel_id: "tunnel".into(),
                control_plane_http_proxy: None,
                tunnel_client_path: None,
                auto_detect: true,
                api_key_ref: None,
            },
            security: harness_control_protocol::SecurityConfig {
                mode: harness_control_protocol::SecurityMode::Normal,
                developer_test_mode: false,
            },
            ui: Default::default(),
            tools: Default::default(),
            bash: Default::default(),
            policies: Default::default(),
            desktop: Default::default(),
            history,
        }
    }

    fn event(index: usize) -> EvidenceEventV1 {
        EvidenceEventV1 {
            schema_version: 1,
            event_id: format!("event-{index}"),
            runtime_id: "runtime-ui".into(),
            request_id: format!("request-{index}"),
            host_session_id: None,
            supervisor_generation_id: None,
            approval_id: None,
            tool_name: "read_file".into(),
            occurred_at_utc: format!("2026-11-01T00:00:0{index}.000Z"),
            recorded_at_utc: format!("2026-11-01T00:00:0{index}.000Z"),
            stage: EvidenceStage::ExecutionCompleted,
            lifecycle_state: harness_control_protocol::EvidenceLifecycleState::Completed,
            workspace_validation: EvidenceWorkspaceValidation::Passed,
            policy_decision: Some(harness_control_protocol::PolicyDecision::Allow),
            policy_reason: Some(harness_control_protocol::PolicyReason::ToolDefault),
            matched_rule_ids: Vec::new(),
            approval_decision: None,
            outcome: HistoryOutcome::Success,
            failure_category: None,
            duration_ms: Some(1),
            sanitized_arguments: None,
            result_evidence: None,
            coverage: EvidenceCoverage::Complete,
        }
    }

    #[test]
    fn desired_config_controls_status_without_exposing_database_path() {
        let config = app_config(HistoryConfig {
            enabled: false,
            retention_days: 12,
            max_storage_mib: 64,
        });
        let status = EvidenceStoreStatus {
            state: EvidenceStoreState::Healthy,
            schema_version: Some(1),
            queue_depth: 3,
            queue_capacity: 1024,
            drop_count: 0,
            last_drop_at_utc: None,
            last_commit_at_utc: None,
            last_error_code: Some(EvidenceErrorCode::EvidenceIo),
            effective_history_config: HistoryConfig::default(),
            footprint_bytes: Some(0),
            coverage_unknown_since_utc: None,
        };
        let result = with_desired_status(status, &config);
        assert_eq!(result.state, EvidenceStoreState::Disabled);
        assert_eq!(result.effective_history_config.retention_days, 12);
        assert_eq!(result.queue_depth, 0);
        assert!(!serde_json::to_string(&result).unwrap().contains("sqlite"));
    }

    #[test]
    fn observability_errors_keep_migration_and_request_failures_distinct() {
        let config = app_config(HistoryConfig::default());
        assert_eq!(
            observability_store_error(StoreError::InvalidEvidence("bad range".into()), &config)
                .code,
            "INVALID_REQUEST"
        );
        assert_eq!(
            observability_store_error(StoreError::NotFound, &config).code,
            "EVIDENCE_NOT_FOUND"
        );
        assert_eq!(
            observability_store_error(StoreError::IncompatibleSchema { found: 1 }, &config).code,
            "EVIDENCE_MIGRATION_REQUIRED"
        );
        assert_eq!(observability_disabled().code, "EVIDENCE_UNAVAILABLE");
    }

    #[test]
    fn read_only_query_works_under_writer_and_offline_clear_requires_ownership() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("history").join("history.sqlite3");
        let workspace_file = directory.path().join("workspace-file.txt");
        std::fs::write(&workspace_file, "workspace sentinel").unwrap();
        let config = app_config(HistoryConfig::default());
        let mut writer = EvidenceWriter::open_with_options(
            &database,
            WriterOptions {
                history_config: config.history.clone(),
                runtime_version: "0.1.3".into(),
                config_revision: "7".into(),
            },
        )
        .unwrap();
        writer
            .open_session(&RuntimeSession {
                runtime_id: "runtime-ui".into(),
                host_session_id: Some("host".into()),
                supervisor_generation_id: Some("generation".into()),
                started_at_utc: "2026-11-01T00:00:00.000Z".into(),
                runtime_version: "0.1.3".into(),
                config_revision: "7".into(),
            })
            .unwrap();
        writer.record_event(&event(1)).unwrap();
        writer.record_event(&event(2)).unwrap();

        let result = query_at(database.clone(), &config, HistoryQueryParams::default()).unwrap();
        assert_eq!(result.items.len(), 2);
        assert!(result.store_status.state == EvidenceStoreState::Healthy);

        // M08 projections share the same read-only facade while this writer
        // holds ownership. Completion alone cannot prove ExecutionStarted.
        let metrics = metrics_query_at(
            database.clone(),
            &config,
            MetricsQueryParams {
                from_utc: "2026-11-01T00:00:00.000Z".into(),
                to_utc: "2026-11-02T00:00:00.000Z".into(),
                tool: None,
            },
        )
        .unwrap();
        assert_eq!(metrics.counts.execution_attempts, 0);
        assert_eq!(metrics.counts.unknown_calls, 2);
        let trace = trace_get_at(
            database.clone(),
            &config,
            TraceGetParams {
                runtime_id: "runtime-ui".into(),
                request_id: "request-1".into(),
            },
        )
        .unwrap();
        assert!(!trace.executor_started);
        assert_eq!(
            trace.availability,
            harness_control_protocol::TraceAvailability::LegacyUntraced
        );
        assert!(trace.spans.is_empty());

        let blocked = clear_offline_at(database.clone(), &config).unwrap_err();
        assert_eq!(blocked.code, "EVIDENCE_BUSY");
        assert!(workspace_file.exists());

        drop(writer);
        let cleared = clear_offline_at(database.clone(), &config).unwrap();
        assert_eq!(cleared.deleted_request_count, 2);
        assert!(workspace_file.exists());
    }
}
