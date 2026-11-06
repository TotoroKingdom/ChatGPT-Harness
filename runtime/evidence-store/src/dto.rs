//! Public data-transfer objects and small value objects used by the evidence DAL.
//!
//! The wire-facing history DTOs live in the protocol crate.  Re-exporting them
//! here keeps callers of this crate independent from the protocol module layout
//! while ensuring that the store and the Tauri facade use exactly one closed,
//! typed representation.

pub use harness_control_protocol::v1::control_read::ActivityFailureCategory;
pub use harness_control_protocol::v1::{
    ApprovalDecision, DurableSanitizedArgumentsEvidence, DurationMetric, EvidenceCoverage,
    EvidenceErrorCode, EvidenceEventV1, EvidenceLifecycleState, EvidenceStage, EvidenceStoreState,
    EvidenceStoreStatus, EvidenceWorkspaceValidation, ExecutionResultEvidence,
    HistoryApprovalEvidence, HistoryApprovalState, HistoryClearResult, HistoryConfig,
    HistoryGetParams, HistoryGetResult, HistoryOutcome, HistoryQueryParams, HistoryQueryResult,
    HistoryRequestSummary, HistoryStoreNotice, HistoryTerminalState, MetricsCounts,
    MetricsCoverage, MetricsQueryParams, MetricsQueryResult, MetricsSampleStatus, PolicyDecision,
    PolicyReason, RequestTraceSpan, TraceAvailability, TraceCoverage, TraceGetParams,
    TraceGetResult, TraceSpanName, TraceSpanStatus,
};

pub const EVIDENCE_SCHEMA_VERSION: u32 = 1;
pub const HISTORY_QUERY_DEFAULT_LIMIT: u32 = 50;
pub const HISTORY_QUERY_MAX_LIMIT: u32 = 200;

/// The Runtime session metadata persisted before the first request event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSession {
    pub runtime_id: String,
    pub host_session_id: Option<String>,
    pub supervisor_generation_id: Option<String>,
    pub started_at_utc: String,
    pub runtime_version: String,
    pub config_revision: String,
}

impl RuntimeSession {
    pub fn new(runtime_id: impl Into<String>, started_at_utc: impl Into<String>) -> Self {
        Self {
            runtime_id: runtime_id.into(),
            host_session_id: None,
            supervisor_generation_id: None,
            started_at_utc: started_at_utc.into(),
            runtime_version: "unknown".to_owned(),
            config_revision: "unknown".to_owned(),
        }
    }
}

/// Runtime-owned settings which are useful to the store but are never written
/// into a Tool argument or returned as a History request DTO.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriterOptions {
    pub history_config: HistoryConfig,
    pub runtime_version: String,
    pub config_revision: String,
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            history_config: HistoryConfig::default(),
            runtime_version: "unknown".to_owned(),
            config_revision: "unknown".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordOutcome {
    pub inserted: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetentionReport {
    pub deleted_request_count: u64,
    pub deleted_event_count: u64,
    pub deleted_approval_count: u64,
    pub age_cleanup_requests: u64,
    pub capacity_cleanup_requests: u64,
    pub capacity_degraded: bool,
    pub footprint_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorePaths {
    pub database: std::path::PathBuf,
    pub lock: std::path::PathBuf,
}

impl StorePaths {
    pub fn from_database(database: impl AsRef<std::path::Path>) -> Self {
        let database = database.as_ref().to_path_buf();
        let lock = database
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("history.writer.lock");
        Self { database, lock }
    }
}
