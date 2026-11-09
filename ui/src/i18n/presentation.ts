import type {
  ActivityEventType as ProtocolActivityEventType,
  ApprovalDecision,
  ApprovalStatus,
  BashClassification,
  BashRiskFlag,
  MachineErrorCode,
  PolicySnapshotStatus,
  PolicyDecision,
  PolicySource,
  PolicyReason,
  RiskLevel,
  SecurityMode,
  RuntimeReadiness,
  StartEligibilityStatus,
  DiagnosticScope,
  DiagnosticStatus,
  StartupOutcome,
  StartupPhase,
  StartupPhaseStatus,
  ToolReadiness as ProtocolToolReadiness,
} from "@protocol/v1";
import type {
  ActivityCategory,
  ActivityEventType as ControlReadActivityEventType,
  ActivityFailureCategory,
  ActivityLifecycleState,
  ActivityOutcome,
  BashClassification as ControlReadBashClassification,
  BashRiskFlag as ControlReadBashRiskFlag,
  ToolAvailability,
  ToolReadiness as ControlReadToolReadiness,
} from "@/services/harness/control-read";
import type { ComponentStatus, OverallStatus } from "@/services/harness/client";
import type { PageKey } from "@/app/router/routes";

/**
 * Translation keys are deliberately returned instead of translated text.
 * This keeps machine values and user-authored strings outside the presentation
 * map and lets React resolve the key using the active locale.
 */
export type TranslationKey = string;

const pageLabels: Record<PageKey, TranslationKey> = {
  overview: "nav.overview",
  tools: "nav.tools",
  policies: "nav.policies",
  activity: "nav.activity",
  approvals: "nav.approvals",
  settings: "nav.settings",
};

const pageEyebrows: Record<PageKey, TranslationKey> = {
  overview: "nav.eyebrows.overview",
  tools: "nav.eyebrows.tools",
  policies: "nav.eyebrows.policies",
  activity: "nav.eyebrows.activity",
  approvals: "nav.eyebrows.approvals",
  settings: "nav.eyebrows.settings",
};

export function pageLabelKey(value: PageKey): TranslationKey {
  return pageLabels[value];
}

export function pageEyebrowKey(value: PageKey): TranslationKey {
  return pageEyebrows[value];
}

const componentStatusKeys: Record<ComponentStatus, TranslationKey> = {
  stopped: "status.stopped",
  starting: "status.starting",
  ready: "status.ready",
  degraded: "status.degraded",
  error: "status.error",
  stopping: "status.stopping",
};

const overallStatusKeys: Record<OverallStatus, TranslationKey> = componentStatusKeys;

export function componentStatusKey(value: ComponentStatus): TranslationKey {
  return componentStatusKeys[value];
}

export function overallStatusKey(value: OverallStatus): TranslationKey {
  return overallStatusKeys[value];
}

const securityModeKeys: Record<SecurityMode, TranslationKey> = {
  normal: "enum.securityMode.normal",
  developer_test: "enum.securityMode.developerTest",
};

export function securityModeKey(value: SecurityMode): TranslationKey {
  return securityModeKeys[value];
}

const runtimeReadinessKeys: Record<RuntimeReadiness, TranslationKey> = {
  not_ready: "status.runtimeReadiness.notReady",
  initializing: "status.runtimeReadiness.initializing",
  ready: "status.runtimeReadiness.ready",
  degraded: "status.runtimeReadiness.degraded",
};

export function runtimeReadinessKey(value: RuntimeReadiness): TranslationKey {
  return runtimeReadinessKeys[value];
}

const startEligibilityKeys: Record<StartEligibilityStatus, TranslationKey> = {
  setup_required: "status.eligibility.setupRequired",
  configuration_invalid: "status.eligibility.configurationInvalid",
  runtime_distribution_invalid: "status.eligibility.runtimeDistributionInvalid",
  tunnel_id_required: "status.eligibility.tunnelIdRequired",
  api_key_required: "status.eligibility.apiKeyRequired",
  tunnel_distribution_invalid: "status.eligibility.tunnelDistributionInvalid",
  ready_to_start: "status.eligibility.readyToStart",
  runtime_already_running: "status.eligibility.runtimeAlreadyRunning",
  harness_already_running: "status.eligibility.harnessAlreadyRunning",
  lifecycle_conflict: "status.eligibility.lifecycleConflict",
};

export function startEligibilityKey(value: StartEligibilityStatus): TranslationKey {
  return startEligibilityKeys[value];
}

export function diagnosticStatusKey(value: DiagnosticStatus): TranslationKey {
  return `diagnostics.status.${value}`;
}

export function diagnosticScopeKey(value: DiagnosticScope): TranslationKey {
  return `diagnostics.scope.${value}`;
}

export function startupOutcomeKey(value: StartupOutcome): TranslationKey {
  return `diagnostics.startupOutcome.${value}`;
}

export function startupPhaseKey(value: StartupPhase): TranslationKey {
  return `diagnostics.phase.${value}`;
}

export function startupPhaseStatusKey(value: StartupPhaseStatus): TranslationKey {
  return `diagnostics.phaseStatus.${value}`;
}

const policySnapshotStatusKeys: Record<PolicySnapshotStatus, TranslationKey> = {
  valid: "enum.policySnapshotStatus.valid",
  invalid: "enum.policySnapshotStatus.invalid",
};

export function policySnapshotStatusKey(value: PolicySnapshotStatus): TranslationKey {
  return policySnapshotStatusKeys[value];
}

const policySourceKeys: Record<PolicySource, TranslationKey> = {
  system: "enum.policySource.system",
  user: "enum.policySource.user",
};

export function policySourceKey(value: PolicySource): TranslationKey {
  return policySourceKeys[value];
}

const policyDecisionKeys: Record<PolicyDecision, TranslationKey> = {
  allow: "enum.policyDecision.allow",
  ask: "enum.policyDecision.ask",
  deny: "enum.policyDecision.deny",
};

export function policyDecisionKey(value: PolicyDecision): TranslationKey {
  return policyDecisionKeys[value];
}

const policyReasonKeys: Record<PolicyReason, TranslationKey> = {
  system_guardrail: "enum.policyReason.systemGuardrail",
  matched_user_rules: "enum.policyReason.matchedUserRules",
  tool_default: "enum.policyReason.toolDefault",
  invalid_policy_snapshot: "enum.policyReason.invalidPolicySnapshot",
  invalid_context: "enum.policyReason.invalidContext",
  unsupported_capability: "enum.policyReason.unsupportedCapability",
  tool_disabled: "enum.policyReason.toolDisabled",
};

export function policyReasonKey(value: PolicyReason): TranslationKey {
  return policyReasonKeys[value];
}

/** The control-read projection includes unavailable_by_mode in addition to
 * the protocol's policy-catalog availability values. */
type PresentationToolAvailability =
  | ToolAvailability
  | "available"
  | "available_with_approval"
  | "unavailable_by_policy"
  | "unavailable_by_runtime"
  | "disabled";

const toolAvailabilityKeys: Record<PresentationToolAvailability, TranslationKey> = {
  available: "enum.toolAvailability.available",
  available_with_approval: "enum.toolAvailability.availableWithApproval",
  unavailable_by_mode: "enum.toolAvailability.unavailableByMode",
  unavailable_by_policy: "enum.toolAvailability.unavailableByPolicy",
  unavailable_by_runtime: "enum.toolAvailability.unavailableByRuntime",
  disabled: "enum.toolAvailability.disabled",
};

export function toolAvailabilityKey(value: PresentationToolAvailability): TranslationKey {
  return toolAvailabilityKeys[value];
}

type PresentationToolReadiness = ProtocolToolReadiness | ControlReadToolReadiness;

const toolReadinessKeys: Record<PresentationToolReadiness, TranslationKey> = {
  ready: "enum.toolReadiness.ready",
  unavailable: "enum.toolReadiness.unavailable",
};

export function toolReadinessKey(value: PresentationToolReadiness): TranslationKey {
  return toolReadinessKeys[value];
}

const riskLevelKeys: Record<RiskLevel, TranslationKey> = {
  low: "enum.riskLevel.low",
  medium: "enum.riskLevel.medium",
  high: "enum.riskLevel.high",
};

export function riskLevelKey(value: RiskLevel): TranslationKey {
  return riskLevelKeys[value];
}

const approvalStatusKeys: Record<ApprovalStatus, TranslationKey> = {
  pending: "enum.approvalStatus.pending",
  approved: "enum.approvalStatus.approved",
  denied: "enum.approvalStatus.denied",
  expired: "enum.approvalStatus.expired",
  cancelled: "enum.approvalStatus.cancelled",
};

export function approvalStatusKey(value: ApprovalStatus): TranslationKey {
  return approvalStatusKeys[value];
}

const approvalDecisionKeys: Record<ApprovalDecision, TranslationKey> = {
  allow_once: "enum.approvalDecision.allowOnce",
  allow_for_session: "enum.approvalDecision.allowForSession",
  deny: "enum.approvalDecision.deny",
};

export function approvalDecisionKey(value: ApprovalDecision): TranslationKey {
  return approvalDecisionKeys[value];
}

const activityCategoryKeys: Record<ActivityCategory, TranslationKey> = {
  request: "enum.activityCategory.request",
  workspace: "enum.activityCategory.workspace",
  policy: "enum.activityCategory.policy",
  execution: "enum.activityCategory.execution",
  approval: "enum.activityCategory.approval",
};

export function activityCategoryKey(value: ActivityCategory): TranslationKey {
  return activityCategoryKeys[value];
}

type PresentationActivityEventType = ProtocolActivityEventType | ControlReadActivityEventType;

const activityEventKeys: Record<PresentationActivityEventType, TranslationKey> = {
  runtime_started: "enum.activityEvent.runtimeStarted",
  runtime_stopped: "enum.activityEvent.runtimeStopped",
  tool_requested: "enum.activityEvent.toolRequested",
  validation_failed: "enum.activityEvent.validationFailed",
  workspace_violation: "enum.activityEvent.workspaceViolation",
  policy_evaluated: "enum.activityEvent.policyEvaluated",
  approval_requested: "enum.activityEvent.approvalRequested",
  approval_required: "enum.activityEvent.approvalRequired",
  approval_resolved: "enum.activityEvent.approvalResolved",
  approval_denied: "enum.activityEvent.approvalDenied",
  approval_expired: "enum.activityEvent.approvalExpired",
  approval_cancelled: "enum.activityEvent.approvalCancelled",
  execution_started: "enum.activityEvent.executionStarted",
  execution_completed: "enum.activityEvent.executionCompleted",
  execution_failed: "enum.activityEvent.executionFailed",
  execution_timed_out: "enum.activityEvent.executionTimedOut",
  execution_cancelled: "enum.activityEvent.executionCancelled",
  request_received: "enum.activityEvent.requestReceived",
  workspace_validated: "enum.activityEvent.workspaceValidated",
  request_denied: "enum.activityEvent.requestDenied",
  session_grant_matched: "enum.activityEvent.sessionGrantMatched",
};

export function activityEventKey(value: PresentationActivityEventType): TranslationKey {
  return activityEventKeys[value];
}

const activityLifecycleStateKeys: Record<ActivityLifecycleState, TranslationKey> = {
  received: "enum.activityLifecycle.received",
  validating: "enum.activityLifecycle.validating",
  workspace_validating: "enum.activityLifecycle.workspaceValidating",
  policy_evaluating: "enum.activityLifecycle.policyEvaluating",
  denied: "enum.activityLifecycle.denied",
  awaiting_approval: "enum.activityLifecycle.awaitingApproval",
  executing: "enum.activityLifecycle.executing",
  completed: "enum.activityLifecycle.completed",
  failed: "enum.activityLifecycle.failed",
};

export function activityLifecycleStateKey(value: ActivityLifecycleState): TranslationKey {
  return activityLifecycleStateKeys[value];
}

const activityOutcomeKeys: Record<ActivityOutcome, TranslationKey> = {
  in_progress: "enum.activityOutcome.inProgress",
  success: "enum.activityOutcome.success",
  failed: "enum.activityOutcome.failed",
  denied: "enum.activityOutcome.denied",
  approval_required: "enum.activityOutcome.approvalRequired",
};

export function activityOutcomeKey(value: ActivityOutcome): TranslationKey {
  return activityOutcomeKeys[value];
}

const activityFailureCategoryKeys: Record<ActivityFailureCategory, TranslationKey> = {
  approval_denied: "enum.activityFailure.approvalDenied",
  approval_expired: "enum.activityFailure.approvalExpired",
  approval_cancelled: "enum.activityFailure.approvalCancelled",
  tool_disabled: "enum.activityFailure.toolDisabled",
  target_changed: "enum.activityFailure.targetChanged",
  atomic_failure: "enum.activityFailure.atomicFailure",
  io_failure: "enum.activityFailure.ioFailure",
  workspace_violation: "enum.activityFailure.workspaceViolation",
  invalid_arguments: "enum.activityFailure.invalidArguments",
  execution_failed: "enum.activityFailure.executionFailed",
  bash_unavailable: "enum.activityFailure.bashUnavailable",
  bash_invalid_command: "enum.activityFailure.bashInvalidCommand",
  bash_command_too_long: "enum.activityFailure.bashCommandTooLong",
  bash_timeout: "enum.activityFailure.bashTimeout",
  bash_output_limit: "enum.activityFailure.bashOutputLimit",
  bash_concurrency_limit: "enum.activityFailure.bashConcurrencyLimit",
  bash_spawn_failed: "enum.activityFailure.bashSpawnFailed",
  bash_cancelled: "enum.activityFailure.bashCancelled",
  bash_process_failed: "enum.activityFailure.bashProcessFailed",
  grep_timeout: "enum.activityFailure.grepTimeout",
  grep_cancelled: "enum.activityFailure.grepCancelled",
  glob_timeout: "enum.activityFailure.globTimeout",
  glob_cancelled: "enum.activityFailure.globCancelled",
};

export function activityFailureCategoryKey(value: ActivityFailureCategory): TranslationKey {
  return activityFailureCategoryKeys[value];
}

type PresentationBashClassification = BashClassification | ControlReadBashClassification;

const bashClassificationKeys: Record<PresentationBashClassification, TranslationKey> = {
  classified: "enum.bashClassification.classified",
  unclassified_high_risk: "enum.bashClassification.unclassifiedHighRisk",
};

export function bashClassificationKey(value: PresentationBashClassification): TranslationKey {
  return bashClassificationKeys[value];
}

type PresentationBashRiskFlag = BashRiskFlag | ControlReadBashRiskFlag;

const bashRiskFlagKeys: Record<PresentationBashRiskFlag, TranslationKey> = {
  command_substitution: "enum.bashRiskFlag.commandSubstitution",
  backticks: "enum.bashRiskFlag.backticks",
  eval: "enum.bashRiskFlag.eval",
  nested_shell: "enum.bashRiskFlag.nestedShell",
  interpreter_inline_code: "enum.bashRiskFlag.interpreterInlineCode",
  redirection: "enum.bashRiskFlag.redirection",
  dynamic_expansion: "enum.bashRiskFlag.dynamicExpansion",
  malformed_quoting: "enum.bashRiskFlag.malformedQuoting",
  unsupported_grammar: "enum.bashRiskFlag.unsupportedGrammar",
};

export function bashRiskFlagKey(value: PresentationBashRiskFlag): TranslationKey {
  return bashRiskFlagKeys[value];
}

/**
 * These include protocol MachineErrorCode values plus errors emitted by the
 * Tauri host, config store and control-read boundary. Keep this record
 * exhaustive for the protocol union; adding a protocol code then forces its
 * presentation mapping to be added here at compile time.
 */
export type HostErrorCode =
  | "APPROVAL_REQUIRED"
  | "APPROVAL_DENIED"
  | "APPROVAL_EXPIRED"
  | "APPROVAL_CANCELLED"
  | "WORKSPACE_VIOLATION"
  | "TOOL_DISABLED"
  | "POLICY_DENIED"
  | "TARGET_NOT_FOUND"
  | "TARGET_NOT_FILE"
  | "TARGET_NOT_DIRECTORY"
  | "UNSUPPORTED_CONTENT"
  | "READ_LIMIT_EXCEEDED"
  | "INVALID_PATTERN"
  | "INVALID_BASE_PATH"
  | "INVALID_REGEX"
  | "INVALID_FILTER_PATTERN"
  | "INVALID_SCOPE"
  | "SEARCH_TIMEOUT"
  | "CANCELLED"
  | "EDIT_MATCH_NOT_FOUND"
  | "EDIT_MATCH_NOT_UNIQUE"
  | "TARGET_CHANGED"
  | "IO_FAILURE"
  | "INVALID_ARGUMENTS"
  | "EXECUTION_FAILED"
  | "CONFIG_SCHEMA_UNSUPPORTED"
  | "CONFIG_MIGRATION_FAILED"
  | "CONFIG_INVALID"
  | "CONFIG_WRITE_FAILED"
  | "CONFIG_NOT_FOUND"
  | "WORKSPACE_INVALID"
  | "PATH_ERROR"
  | "SECRET_STORE_FAILED"
  | "TUNNEL_CLIENT_NOT_FOUND"
  | "RUNTIME_UNAVAILABLE"
  | "RUNTIME_NOT_INITIALIZED"
  | "RUNTIME_DISCONNECTED"
  | "RUNTIME_TIMEOUT"
  | "RUNTIME_GRACEFUL_STOP_TIMEOUT"
  | "RUNTIME_DISTRIBUTION_INVALID"
  | "RUNTIME_SPAWN_FAILED"
  | "RUNTIME_CRASHED"
  | "RUNTIME_INVALID_RESPONSE"
  | "RUNTIME_KILL_FAILED"
  | "RUNTIME_NOT_READY"
  | "INVALID_REQUEST"
  | "CONTROL_READ_FAILED"
  | "LIFECYCLE_CONFLICT"
  | "TUNNEL_DISTRIBUTION_INVALID"
  | "TUNNEL_LAUNCH_INVALID"
  | "TUNNEL_EXECUTABLE_NOT_FOUND"
  | "TUNNEL_SPAWN_FAILED"
  | "TUNNEL_INSTANCE_MISMATCH"
  | "TUNNEL_STATE_UNAVAILABLE"
  | "TUNNEL_STOP_FAILED"
  | "TUNNEL_NOT_READY"
  | "CONTROL_PLANE_UNAUTHORIZED"
  | "CONTROL_PLANE_UNAVAILABLE"
  | "TUNNEL_PROCESS_EXITED"
  | "HARNESS_LIFECYCLE_FAILED"
  | "CONFIG_REVISION_OVERFLOW";

export type KnownErrorCode = MachineErrorCode | HostErrorCode;

const errorKeys: Record<KnownErrorCode, TranslationKey> = {
  NOT_INITIALIZED: "errors.codes.notInitialized",
  ALREADY_INITIALIZED: "errors.codes.alreadyInitialized",
  PROTOCOL_VERSION_MISMATCH: "errors.codes.protocolVersionMismatch",
  RUNTIME_NOT_READY: "errors.codes.runtimeNotReady",
  INVALID_ARGUMENT: "errors.codes.invalidArgument",
  APPROVAL_NOT_FOUND: "errors.codes.approvalNotFound",
  APPROVAL_ALREADY_RESOLVED: "errors.codes.approvalAlreadyResolved",
  APPROVAL_SESSION_GRANT_UNAVAILABLE: "errors.codes.approvalSessionGrantUnavailable",
  BASH_UNAVAILABLE: "errors.codes.bashUnavailable",
  BASH_INVALID_COMMAND: "errors.codes.bashInvalidCommand",
  BASH_COMMAND_TOO_LONG: "errors.codes.bashCommandTooLong",
  BASH_TIMEOUT: "errors.codes.bashTimeout",
  BASH_OUTPUT_LIMIT: "errors.codes.bashOutputLimit",
  BASH_CONCURRENCY_LIMIT: "errors.codes.bashConcurrencyLimit",
  BASH_SPAWN_FAILED: "errors.codes.bashSpawnFailed",
  BASH_CANCELLED: "errors.codes.bashCancelled",
  BASH_PROCESS_FAILED: "errors.codes.bashProcessFailed",
  EVIDENCE_BUSY: "errors.codes.evidenceBusy",
  EVIDENCE_WRITER_OWNERSHIP: "errors.codes.evidenceWriterOwnership",
  EVIDENCE_IO: "errors.codes.evidenceIo",
  EVIDENCE_FULL: "errors.codes.evidenceFull",
  EVIDENCE_CORRUPT: "errors.codes.evidenceCorrupt",
  EVIDENCE_INCOMPATIBLE_SCHEMA: "errors.codes.evidenceIncompatibleSchema",
  EVIDENCE_MIGRATION_FAILED: "errors.codes.evidenceMigrationFailed",
  EVIDENCE_QUEUE_OVERFLOW: "errors.codes.evidenceQueueOverflow",
  EVIDENCE_CAPACITY_DEGRADED: "errors.codes.evidenceCapacityDegraded",
  EVIDENCE_INVALID_CURSOR: "errors.codes.evidenceInvalidCursor",
  EVIDENCE_NOT_FOUND: "errors.codes.evidenceNotFound",
  INTERNAL_ERROR: "errors.codes.internalError",
  APPROVAL_REQUIRED: "errors.codes.approvalRequired",
  APPROVAL_DENIED: "errors.codes.approvalDenied",
  APPROVAL_EXPIRED: "errors.codes.approvalExpired",
  APPROVAL_CANCELLED: "errors.codes.approvalCancelled",
  WORKSPACE_VIOLATION: "errors.codes.workspaceViolation",
  TOOL_DISABLED: "errors.codes.toolDisabled",
  POLICY_DENIED: "errors.codes.policyDenied",
  TARGET_NOT_FOUND: "errors.codes.targetNotFound",
  TARGET_NOT_FILE: "errors.codes.targetNotFile",
  TARGET_NOT_DIRECTORY: "errors.codes.targetNotDirectory",
  UNSUPPORTED_CONTENT: "errors.codes.unsupportedContent",
  READ_LIMIT_EXCEEDED: "errors.codes.readLimitExceeded",
  INVALID_PATTERN: "errors.codes.invalidPattern",
  INVALID_BASE_PATH: "errors.codes.invalidBasePath",
  INVALID_REGEX: "errors.codes.invalidRegex",
  INVALID_FILTER_PATTERN: "errors.codes.invalidFilterPattern",
  INVALID_SCOPE: "errors.codes.invalidScope",
  SEARCH_TIMEOUT: "errors.codes.searchTimeout",
  CANCELLED: "errors.codes.searchCancelled",
  EDIT_MATCH_NOT_FOUND: "errors.codes.editMatchNotFound",
  EDIT_MATCH_NOT_UNIQUE: "errors.codes.editMatchNotUnique",
  TARGET_CHANGED: "errors.codes.targetChanged",
  IO_FAILURE: "errors.codes.ioFailure",
  INVALID_ARGUMENTS: "errors.codes.invalidArguments",
  EXECUTION_FAILED: "errors.codes.executionFailed",
  CONFIG_SCHEMA_UNSUPPORTED: "errors.codes.configSchemaUnsupported",
  CONFIG_MIGRATION_FAILED: "errors.codes.configMigrationFailed",
  CONFIG_INVALID: "errors.codes.configInvalid",
  CONFIG_WRITE_FAILED: "errors.codes.configWriteFailed",
  CONFIG_NOT_FOUND: "errors.codes.configNotFound",
  WORKSPACE_INVALID: "errors.codes.workspaceInvalid",
  PATH_ERROR: "errors.codes.pathError",
  SECRET_STORE_FAILED: "errors.codes.secretStoreFailed",
  TUNNEL_CLIENT_NOT_FOUND: "errors.codes.tunnelClientNotFound",
  RUNTIME_UNAVAILABLE: "errors.codes.runtimeUnavailable",
  RUNTIME_NOT_INITIALIZED: "errors.codes.runtimeNotInitialized",
  RUNTIME_DISCONNECTED: "errors.codes.runtimeDisconnected",
  RUNTIME_TIMEOUT: "errors.codes.runtimeTimeout",
  RUNTIME_GRACEFUL_STOP_TIMEOUT: "errors.codes.runtimeGracefulStopTimeout",
  RUNTIME_DISTRIBUTION_INVALID: "errors.codes.runtimeDistributionInvalid",
  RUNTIME_SPAWN_FAILED: "errors.codes.runtimeSpawnFailed",
  RUNTIME_CRASHED: "errors.codes.runtimeCrashed",
  RUNTIME_INVALID_RESPONSE: "errors.codes.runtimeInvalidResponse",
  RUNTIME_KILL_FAILED: "errors.codes.runtimeKillFailed",
  INVALID_REQUEST: "errors.codes.invalidRequest",
  CONTROL_READ_FAILED: "errors.codes.controlReadFailed",
  LIFECYCLE_CONFLICT: "errors.codes.lifecycleConflict",
  TUNNEL_DISTRIBUTION_INVALID: "errors.codes.tunnelDistributionInvalid",
  TUNNEL_LAUNCH_INVALID: "errors.codes.tunnelLaunchInvalid",
  TUNNEL_EXECUTABLE_NOT_FOUND: "errors.codes.tunnelExecutableNotFound",
  TUNNEL_SPAWN_FAILED: "errors.codes.tunnelSpawnFailed",
  TUNNEL_INSTANCE_MISMATCH: "errors.codes.tunnelInstanceMismatch",
  TUNNEL_STATE_UNAVAILABLE: "errors.codes.tunnelStateUnavailable",
  TUNNEL_STOP_FAILED: "errors.codes.tunnelStopFailed",
  TUNNEL_NOT_READY: "errors.codes.tunnelNotReady",
  CONTROL_PLANE_UNAUTHORIZED: "errors.codes.controlPlaneUnauthorized",
  CONTROL_PLANE_UNAVAILABLE: "errors.codes.controlPlaneUnavailable",
  TUNNEL_PROCESS_EXITED: "errors.codes.tunnelProcessExited",
  HARNESS_LIFECYCLE_FAILED: "errors.codes.harnessLifecycleFailed",
  CONFIG_REVISION_OVERFLOW: "errors.codes.configRevisionOverflow",
};

export const knownErrorCodeKeys = errorKeys;

function stringCode(error: unknown): string | undefined {
  if (typeof error === "string") return /^[A-Z][A-Z0-9_]{0,127}$/.test(error) ? error : undefined;
  if (!error || typeof error !== "object" || !("code" in error)) return undefined;
  const value = (error as { code?: unknown }).code;
  return typeof value === "string" && /^[A-Z][A-Z0-9_]{0,127}$/.test(value) ? value : undefined;
}

/** Return a localized generic failure for unknown codes; never infer success. */
export function errorCodeKey(errorOrCode: unknown): TranslationKey {
  const code = stringCode(errorOrCode);
  // RuntimeSupervisor wraps protocol failures as RPC_<machine code>. Only
  // presentation resolves this envelope; retain the original code in data.
  const machineCode = code?.startsWith("RPC_") ? code.slice(4) : code;
  return machineCode && Object.prototype.hasOwnProperty.call(errorKeys, machineCode)
    ? errorKeys[machineCode as KnownErrorCode]
    : "errors.unknown";
}

export type ErrorPresentation = {
  summaryKey: TranslationKey;
  code?: string;
  detail?: string;
};

function safeDetail(value: unknown): string | undefined {
  if (typeof value !== "string" || value.length > 1024 || /[\u0000-\u001f\u007f]/.test(value)) return undefined;
  return value;
}

/**
 * Normalize a service error for React. The service may pass an already
 * sanitized fallback message; arbitrary thrown values are reduced to a
 * generic localized key and never become user-facing prose by inference.
 */
export function presentError(error: unknown, sanitizedDetail?: string): ErrorPresentation {
  const code = stringCode(error);
  const detail = safeDetail(sanitizedDetail);
  return {
    summaryKey: errorCodeKey(code),
    ...(code ? { code } : {}),
    ...(detail ? { detail } : {}),
  };
}

const systemPolicyMap: Record<"system-read-workspace" | "system-deny-outside", { name: TranslationKey; reason: TranslationKey }> = {
  "system-read-workspace": {
    name: "policy.system.systemReadWorkspace.name",
    reason: "policy.system.systemReadWorkspace.reason",
  },
  "system-deny-outside": {
    name: "policy.system.systemDenyOutside.name",
    reason: "policy.system.systemDenyOutside.reason",
  },
};

export type SystemPolicyPresentation = { name: TranslationKey; reason: TranslationKey };

export function systemPolicyKeys(ruleId: string): SystemPolicyPresentation {
  return systemPolicyMap[ruleId as keyof typeof systemPolicyMap] ?? {
    name: "policy.system.unknownName",
    reason: "policy.system.unknownReason",
  };
}

export function systemPolicyNameKey(ruleId: string): TranslationKey {
  return systemPolicyKeys(ruleId).name;
}

export function systemPolicyReasonKey(ruleId: string): TranslationKey {
  return systemPolicyKeys(ruleId).reason;
}

const toolReasonKeys: Record<import("@protocol/v1").ToolStatusReason, TranslationKey> = {
  available: "m7.reasons.available",
  approval_required: "m7.reasons.approval_required",
  tool_disabled: "m7.reasons.tool_disabled",
  developer_mode_required: "m7.reasons.developer_mode_required",
  runtime_not_ready: "m7.reasons.runtime_not_ready",
  capability_unavailable: "m7.reasons.capability_unavailable",
  bash_executable_unavailable: "m7.reasons.bash_executable_unavailable",
  policy_denied: "m7.reasons.policy_denied",
  invalid_policy_snapshot: "m7.reasons.invalid_policy_snapshot",
  invalid_policy_context: "m7.reasons.invalid_policy_context",
  unsupported_capability: "m7.reasons.unsupported_capability",
  metadata_invalid: "m7.reasons.metadata_invalid",
};
const toolRemediationKeys: Record<import("@protocol/v1").ToolStatusReason, TranslationKey> = {
  available: "m7.remediation.available",
  approval_required: "m7.remediation.approval_required",
  tool_disabled: "m7.remediation.tool_disabled",
  developer_mode_required: "m7.remediation.developer_mode_required",
  runtime_not_ready: "m7.remediation.runtime_not_ready",
  capability_unavailable: "m7.remediation.capability_unavailable",
  bash_executable_unavailable: "m7.remediation.bash_executable_unavailable",
  policy_denied: "m7.remediation.policy_denied",
  invalid_policy_snapshot: "m7.remediation.invalid_policy_snapshot",
  invalid_policy_context: "m7.remediation.invalid_policy_context",
  unsupported_capability: "m7.remediation.unsupported_capability",
  metadata_invalid: "m7.remediation.metadata_invalid",
};
export const toolStatusReasonKey = (reason: import("@protocol/v1").ToolStatusReason) => toolReasonKeys[reason];
export const toolRemediationKey = (reason: import("@protocol/v1").ToolStatusReason) => toolRemediationKeys[reason];
