// GENERATED FILE — DO NOT EDIT.
// Source: harness-control-protocol Rust models.

export type RpcRequest = { id: number | string, method: string, params?: unknown, };
export type RpcResponse = { id: number | string, result: unknown, };
export type RpcNotification = { method: string, params?: unknown, };
export type RpcErrorData = {
/**
 * Stable machine-readable error code. Do not parse `message`.
 */
code: MachineErrorCode, details?: string, };
export type RpcError = { code: number, message: string, data: RpcErrorData, };
export type RpcErrorResponse = { id: number | string, error: RpcError, };
export type RpcMessage = RpcErrorResponse | RpcResponse | RpcRequest | RpcNotification;
export type MachineErrorCode = "NOT_INITIALIZED" | "ALREADY_INITIALIZED" | "PROTOCOL_VERSION_MISMATCH" | "RUNTIME_NOT_READY" | "INVALID_ARGUMENT" | "APPROVAL_NOT_FOUND" | "APPROVAL_ALREADY_RESOLVED" | "APPROVAL_SESSION_GRANT_UNAVAILABLE" | "BASH_UNAVAILABLE" | "BASH_INVALID_COMMAND" | "BASH_COMMAND_TOO_LONG" | "BASH_TIMEOUT" | "BASH_OUTPUT_LIMIT" | "BASH_CONCURRENCY_LIMIT" | "BASH_SPAWN_FAILED" | "BASH_CANCELLED" | "BASH_PROCESS_FAILED" | "EVIDENCE_BUSY" | "EVIDENCE_WRITER_OWNERSHIP" | "EVIDENCE_IO" | "EVIDENCE_FULL" | "EVIDENCE_CORRUPT" | "EVIDENCE_INCOMPATIBLE_SCHEMA" | "EVIDENCE_MIGRATION_FAILED" | "EVIDENCE_QUEUE_OVERFLOW" | "EVIDENCE_CAPACITY_DEGRADED" | "EVIDENCE_INVALID_CURSOR" | "EVIDENCE_NOT_FOUND" | "INTERNAL_ERROR";
export type EmptyParams = Record<string, never>;
export type PolicyDecision = "deny" | "ask" | "allow";
export type AppConfig = { schemaVersion: number, configRevision: string, workspace: WorkspaceConfig, tunnel: TunnelConfig, security: SecurityConfig, ui: UiConfig, tools: ToolConfig, bash: BashConfig, policies: PolicyConfig,
/**
 * Host-only desktop presentation and lifecycle preferences. These fields
 * are deliberately excluded from `RuntimeConfigSnapshot`.
 */
desktop: DesktopConfig,
/**
 * Local History persistence preferences. Changes are generation-effective
 * and never part of `RuntimeConfigSnapshot` or authorization truth.
 */
history: HistoryConfig, };
export type DesktopConfig = { approvalNotifications: boolean, connectionNotifications: boolean, backgroundNoticeAcknowledged: boolean, autoStartHarnessOnAppLaunch: boolean, };
export type HistoryConfig = { enabled: boolean, retentionDays: number, maxStorageMiB: number, };
export type ToolPolicyDefaults = { readFile: PolicyDecision, glob: PolicyDecision, write: PolicyDecision, edit: PolicyDecision, bash: PolicyDecision, };
export type PolicyConfig = { toolDefaults: ToolPolicyDefaults, userRules: Array<UserPolicyRuleConfig>, };
export type ToolEnabledConfig = { enabled: boolean, };
export type ToolConfig = { readFile: ToolEnabledConfig, glob: ToolEnabledConfig, write: ToolEnabledConfig, edit: ToolEnabledConfig, bash: ToolEnabledConfig, };
export type BashConfig = { autoDetect: boolean, executablePath?: string, };
export type WorkspaceConfig = { rootPath: string, };
export type TunnelConfig = { tunnelId: string, controlPlaneHttpProxy?: string, tunnelClientPath?: string, autoDetect: boolean, apiKeyRef?: SecretReference, };
export type SecretReference = { provider: string, key: string, };
export type SecurityMode = "normal" | "developer_test";
export type SecurityConfig = { mode: SecurityMode, developerTestMode: boolean, };
export type UiLanguage = "en" | "zh-CN";
export type UiConfig = { theme?: string, language?: UiLanguage, };
export type StartEligibilityStatus = "setup_required" | "configuration_invalid" | "runtime_distribution_invalid" | "tunnel_id_required" | "api_key_required" | "tunnel_distribution_invalid" | "ready_to_start" | "runtime_already_running" | "harness_already_running" | "lifecycle_conflict";
export type StartEligibility = { status: StartEligibilityStatus, canStart: boolean, message: string, };
export type RuntimeConfigSnapshot = { configRevision: string, workspace: WorkspaceConfig, security: SecurityConfig, runtimeLimits: RuntimeLimits, tools: ToolConfig, bash: BashConfig, policies: PolicyConfig, };
export type RuntimeLimits = { maxConcurrentRequests: number, maxActivityEvents: number, maxReadBytes: number, };
export type ClientInfo = { name: string, title: string, version: string, };
export type ServerInfo = { name: string, title: string, version: string, };
export type InitializeParams = { clientInfo: ClientInfo, protocolVersion: number, capabilities: { [key in string]?: boolean }, runtimeConfig: RuntimeConfigSnapshot, hostContext?: RuntimeHostContext, };
export type InitializeResult = { serverInfo: ServerInfo, protocolVersion: number, runtimeId: string, capabilities: { [key in string]?: boolean }, };
export type InitializedParams = Record<string, never>;
export type RuntimeLifecycle = "initializing" | "ready" | "stopping" | "error";
export type RuntimeReadiness = "not_ready" | "initializing" | "ready" | "degraded";
export type RuntimeErrorSummary = { code: string, message: string, };
export type RuntimeState = { runtimeId: string,
/**
 * RFC3339 UTC timestamp.
 */
startedAt: string, lifecycle: RuntimeLifecycle, readiness: RuntimeReadiness, configRevision: string, activeWorkspace: string, securityMode: SecurityMode, exposedToolCatalog: Array<ToolDescriptor>, activeRequests: number, pendingApprovals: number, runningExecutions: number, lastError?: RuntimeErrorSummary, };
export type RuntimeStopResult = { accepted: boolean, };
export type McpEndpointResult = { url: string, };
export type ToolReadiness = "ready" | "unavailable";
export type ToolStatusReason = "available" | "approval_required" | "tool_disabled" | "developer_mode_required" | "runtime_not_ready" | "capability_unavailable" | "bash_executable_unavailable" | "policy_denied" | "invalid_policy_snapshot" | "invalid_policy_context" | "unsupported_capability" | "metadata_invalid";
export type RiskLevel = "low" | "medium" | "high";
export type ToolDescriptor = { name: string, description: string, implemented: boolean, enabled: boolean, exposed: boolean, effectivePolicy: PolicyDecision, scope: string, readiness: ToolReadiness, riskLevel: RiskLevel, };
export type ToolsListResult = { items: Array<ToolDescriptor>, };
export type PolicySource = "system" | "user";
export type PolicyRule = { id: string, name: string, decision: PolicyDecision, scope: string, reason: string, source: PolicySource, editable: boolean, enabled: boolean, };
export type PolicyMatcherType = "tool" | "bash_command";
export type ToolMatcherConfig = { tool: string, };
export type BashCommandMatcherConfig = { executable: string, argvPrefix: Array<string>, };
export type PolicyMatcherConfig = { "type": "tool", tool: string, } | { "type": "bash_command", executable: string, argvPrefix: Array<string>, };
export type UserPolicyRuleConfig = { id: string, name: string, enabled: boolean, decision: PolicyDecision, matcher: PolicyMatcherConfig, reason: string, };
export type PolicySnapshotStatus = "valid" | "invalid";
export type PolicyReason = "system_guardrail" | "matched_user_rules" | "tool_default" | "invalid_policy_snapshot" | "invalid_context" | "unsupported_capability" | "tool_disabled";
export type EffectivePolicyRule = { id: string, name: string, decision: PolicyDecision, matcher?: PolicyMatcherConfig, reason: string, source: PolicySource, editable: boolean, enabled: boolean, };
export type PolicyToolAvailability = "available" | "available_with_approval" | "unavailable_by_policy" | "unavailable_by_runtime" | "disabled";
export type EffectivePolicyTool = { tool: string, decision: PolicyDecision, matchedRuleIds: Array<string>, reason: PolicyReason, availability: PolicyToolAvailability, };
export type PolicyEffectiveSnapshot = { valid: boolean, status: PolicySnapshotStatus, statusReason?: PolicyReason, systemRules: Array<EffectivePolicyRule>, userRules: Array<EffectivePolicyRule>, toolDefaults: ToolPolicyDefaults, perTool: Array<EffectivePolicyTool>, };
export type PoliciesEffectiveResult = { snapshot: PolicyEffectiveSnapshot, };
export type PoliciesListResult = { items: Array<PolicyRule>, effective?: PolicyEffectiveSnapshot, };
export type SanitizedArgumentsEvidence = { "tool": "read_file", logicalPath: string, } | { "tool": "glob", pattern: string, } | { "tool": "write", logicalPath: string, } | { "tool": "edit", logicalPath: string, } | { "tool": "bash", commandPreview: string, };
export type ExecutionResultEvidence = { "tool": "read_file", bytesRead: number, } | { "tool": "glob", matchCount: number, } | { "tool": "write", bytesWritten: number, disposition: WriteDisposition, } | { "tool": "edit", replacements: number, bytesWritten: number, } | { "tool": "bash", exitCode: number, };
export type WriteDisposition = "created" | "overwritten";
export type AuditEventType = "request_received" | "workspace_validated" | "policy_evaluated" | "execution_started" | "execution_completed" | "execution_failed" | "request_denied" | "approval_required" | "approval_resolved" | "approval_expired" | "approval_cancelled" | "session_grant_matched";
export type AuditLifecycleState = "received" | "validating" | "workspace_validating" | "policy_evaluating" | "denied" | "awaiting_approval" | "executing" | "completed" | "failed";
export type AuditOutcome = "in_progress" | "success" | "failed" | "denied" | "approval_required";
export type ActivityCategory = "request" | "workspace" | "policy" | "execution" | "approval";
export type ActivityFailureCategory = "approval_denied" | "approval_expired" | "approval_cancelled" | "tool_disabled" | "target_changed" | "atomic_failure" | "io_failure" | "workspace_violation" | "invalid_arguments" | "execution_failed" | "bash_unavailable" | "bash_invalid_command" | "bash_command_too_long" | "bash_timeout" | "bash_output_limit" | "bash_concurrency_limit" | "bash_spawn_failed" | "bash_cancelled" | "bash_process_failed";
export type BashActivityEvidence = { mode: SecurityMode, modeEligible: boolean, capability: ToolCapability, cwd: BashWorkingDirectory, commandPreview?: string | null, classification?: BashClassification | null, riskFlags?: Array<BashRiskFlag>, approvalRequestId?: string | null, sessionGrantHit: boolean, exitCode?: number | null, durationMs?: number, };
export type ToolCapability = "workspace_read" | "workspace_write" | "developer_shell";
export type ActivityEntry = { eventId: string, requestId: string,
/**
 * Normalized UTC; None when the source timestamp is invalid.
 */
timestamp: string | null, category: ActivityCategory, eventType: AuditEventType, lifecycleState: AuditLifecycleState,
/**
 * Only a source-controlled registry identifier, otherwise None.
 */
toolName: string | null,
/**
 * None means policy was not evaluated, never an implicit ALLOW.
 */
decisionSummary: PolicyDecision | null, matchedRuleIds?: Array<string>, policyReason?: PolicyReason | null, outcomeSummary: AuditOutcome, failureCategory?: ActivityFailureCategory | null, bash?: BashActivityEvidence | null, toolEnabled?: boolean, capability?: ToolCapability, sanitizedArguments?: SanitizedArgumentsEvidence, approvalId?: string, approvalDecision?: ApprovalDecision, durationMs?: number, resultEvidence?: ExecutionResultEvidence, };
export type ActivityEventType = "runtime_started" | "runtime_stopped" | "tool_requested" | "validation_failed" | "workspace_violation" | "policy_evaluated" | "approval_requested" | "approval_resolved" | "approval_denied" | "approval_expired" | "approval_cancelled" | "session_grant_matched" | "execution_started" | "execution_completed" | "execution_failed" | "execution_timed_out" | "execution_cancelled";
export type ActivityEvent = { eventId: string, runtimeId: string,
/**
 * Runtime-local monotonically increasing sequence number.
 */
sequence: string, eventType: ActivityEventType, requestId?: string,
/**
 * RFC3339 UTC timestamp.
 */
timestamp: string, summary: string, metadata: { [key in string]?: string }, };
export type ActivityListParams = { afterSequence?: string, limit?: number, };
export type ActivityListResult = { items: Array<ActivityEvent>, latestSequence: string, };
export type ActivitySubscribeParams = { afterSequence?: string, };
export type ActivitySubscribeResult = { latestSequence: string, };
export type EvidenceCoverage = "COMPLETE" | "INCOMPLETE" | "UNKNOWN";
export type HistoryTerminalState = "open" | "succeeded" | "failed" | "denied" | "cancelled" | "timed_out" | "approval_expired" | "approval_cancelled" | "incomplete" | "unknown";
export type HistoryApprovalState = "none" | "required_observed" | "allow_once" | "allow_for_session" | "denied" | "expired" | "cancelled" | "incomplete" | "unknown";
export type HistoryOutcome = "in_progress" | "success" | "failed" | "denied" | "approval_required" | "cancelled" | "timed_out" | "approval_expired" | "approval_cancelled";
export type EvidenceStage = "request_received" | "workspace_validated" | "policy_evaluated" | "approval_required" | "approval_resolved" | "approval_expired" | "approval_cancelled" | "session_grant_matched" | "execution_started" | "execution_completed" | "execution_failed" | "request_denied";
export type EvidenceLifecycleState = "received" | "validating" | "workspace_validating" | "policy_evaluating" | "denied" | "awaiting_approval" | "executing" | "completed" | "failed";
export type EvidenceWorkspaceValidation = "not_attempted" | "passed" | "failed";
export type DurableSanitizedArgumentsEvidence = { "tool": "read_file", logicalPath: string, } | { "tool": "glob", pattern: string, } | { "tool": "write", logicalPath: string, } | { "tool": "edit", logicalPath: string, } | { "tool": "bash", mode: SecurityMode, modeEligible: boolean, capability: ToolCapability, cwd: BashWorkingDirectory, classification?: BashClassification, riskFlags?: Array<BashRiskFlag>, sessionGrantHit: boolean, };
export type EvidenceStoreLaunchConfig = { enabled: boolean, databasePath: string, retentionDays: number, maxStorageMiB: number, };
export type RuntimeHostContext = { hostSessionId?: string, supervisorGenerationId?: string, evidenceStore?: EvidenceStoreLaunchConfig, };
export type EvidenceEventV1 = { schemaVersion: number, eventId: string, runtimeId: string, requestId: string, hostSessionId?: string, supervisorGenerationId?: string, approvalId?: string, toolName: string, occurredAtUtc: string, recordedAtUtc: string, stage: EvidenceStage, lifecycleState: EvidenceLifecycleState, workspaceValidation: EvidenceWorkspaceValidation, policyDecision?: PolicyDecision, policyReason?: PolicyReason, matchedRuleIds?: Array<string>, approvalDecision?: ApprovalDecision, outcome: HistoryOutcome, failureCategory?: ActivityFailureCategory, durationMs?: number, sanitizedArguments?: DurableSanitizedArgumentsEvidence, resultEvidence?: ExecutionResultEvidence, coverage: EvidenceCoverage, };
export type HistoryQueryParams = { fromUtc?: string, toUtc?: string, tool?: string, outcome?: HistoryOutcome, policyDecision?: PolicyDecision, approvalState?: HistoryApprovalState, cursor?: string, limit?: number, };
export type HistoryRequestSummary = { runtimeId: string, requestId: string, toolName: string, firstSeenAtUtc: string, lastSeenAtUtc: string, terminalState: HistoryTerminalState, outcome?: HistoryOutcome, policyDecision?: PolicyDecision, approvalState: HistoryApprovalState, failureCategory?: ActivityFailureCategory, durationMs?: number, evidenceCoverage: EvidenceCoverage, };
export type HistoryGetParams = { runtimeId: string, requestId: string, };
export type HistoryApprovalEvidence = { runtimeId: string, approvalId: string, requestId: string, toolName: string, requestedAtUtc?: string, terminalAtUtc: string, terminalState: HistoryApprovalState, decision?: ApprovalDecision, };
export type HistoryStoreNotice = { code: EvidenceErrorCode, coverage: EvidenceCoverage, };
export type HistoryQueryResult = { items: Array<HistoryRequestSummary>, nextCursor?: string, storeStatus: EvidenceStoreStatus, };
export type HistoryGetResult = { summary: HistoryRequestSummary, events: Array<EvidenceEventV1>, terminalApproval?: HistoryApprovalEvidence, notice?: HistoryStoreNotice, storeStatus: EvidenceStoreStatus, };
export type HistoryClearResult = { deletedRequestCount: number, deletedEventCount: number, deletedApprovalCount: number, footprintBytes: number, completedAtUtc: string, };
export type EvidenceStoreState = "disabled" | "healthy" | "degraded" | "busy" | "incompatible" | "corrupt" | "unavailable";
export type EvidenceErrorCode = "EVIDENCE_BUSY" | "EVIDENCE_WRITER_OWNERSHIP" | "EVIDENCE_IO" | "EVIDENCE_FULL" | "EVIDENCE_CORRUPT" | "EVIDENCE_INCOMPATIBLE_SCHEMA" | "EVIDENCE_MIGRATION_FAILED" | "EVIDENCE_QUEUE_OVERFLOW" | "EVIDENCE_CAPACITY_DEGRADED" | "EVIDENCE_INVALID_CURSOR" | "EVIDENCE_NOT_FOUND";
export type EvidenceStoreStatus = { state: EvidenceStoreState, schemaVersion?: number, queueDepth: number, queueCapacity: number, dropCount: number, lastDropAtUtc?: string, lastCommitAtUtc?: string, lastErrorCode?: EvidenceErrorCode, effectiveHistoryConfig: HistoryConfig, footprintBytes?: number, coverageUnknownSinceUtc?: string, };
export type ApprovalStatus = "pending" | "approved" | "denied" | "expired" | "cancelled";
export type ApprovalDecision = "allow_once" | "allow_for_session" | "deny";
export type BashClassification = "classified" | "unclassified_high_risk";
export type BashRiskFlag = "command_substitution" | "backticks" | "eval" | "nested_shell" | "interpreter_inline_code" | "redirection" | "dynamic_expansion" | "malformed_quoting" | "unsupported_grammar";
export type BashWorkingDirectory = "active_workspace";
export type BashApprovalSegment = {
/**
 * Sanitized normalized executable identity.  This is never a raw shell
 * source fragment or a canonical filesystem path.
 */
executable: string,
/**
 * Logical argv suitable for display only.  A value of `true` in
 * `argv_redacted` means this vector cannot be used to create an exact
 * BashCommand matcher draft.
 */
argv: Array<string>, argvRedacted: boolean, };
export type BashApprovalDetails = {
/**
 * Bounded, shell-display-safe preview; it is never the raw command.
 */
commandPreview: string, classification: BashClassification, normalizedSegments: Array<BashApprovalSegment>, riskFlags: Array<BashRiskFlag>, cwd: BashWorkingDirectory, previewRedacted: boolean, previewTruncated: boolean, };
export type BashSessionGrantUnavailableReason = "unclassified_high_risk";
export type ApprovalRequest = { requestId: string, runtimeId: string, generationId: string, tool: string, sanitizedArguments: { [key in string]?: string }, scopeSummary: string, matchedRuleIds: Array<string>, policyReason: PolicyReason, riskLevel: RiskLevel,
/**
 * Optional M5 Bash projection.  Omitted for existing file-tool approval
 * records so older consumers retain their current wire shape.
 */
bashDetails?: BashApprovalDetails,
/**
 * `None` is the legacy/non-Bash value.  Bash always supplies `Some` so
 * unclassified commands can explicitly disable session grants.
 */
allowForSessionEligible?: boolean, allowForSessionUnavailableReason?: BashSessionGrantUnavailableReason, createdAt: string, expiresAt: string, status: ApprovalStatus, resolvedAt?: string, decision?: ApprovalDecision, };
export type ApprovalsListParams = { status?: ApprovalStatus, };
export type ApprovalsListResult = { items: Array<ApprovalRequest>, };
export type ApprovalsResolveParams = { requestId: string, decision: ApprovalDecision, };
export type ApprovalsResolveResult = { approval: ApprovalRequest, };
export type DiagnosticStatus = "pass" | "warning" | "fail" | "unknown";
export type DiagnosticScope = "config" | "workspace" | "runtime" | "mcp" | "tunnel" | "control_plane" | "proxy" | "credential" | "lifecycle";
export type DiagnosticEvidenceKey = "configured" | "exists" | "version" | "process_phase" | "lifecycle" | "readiness" | "route_mode" | "proxy_source" | "last_success_age_seconds" | "error_code" | "endpoint_reachable";
export type DiagnosticEvidence = { key: DiagnosticEvidenceKey,
/**
 * Values are bounded and sanitized by the Host coordinator.
 */
value: string, };
export type DiagnosticRemediationTarget = "overview" | "settings_workspace" | "settings_connection" | "settings_security";
export type DiagnosticRemediation = { code: string, summary: string, target?: DiagnosticRemediationTarget, };
export type DiagnosticCheck = { id: string, status: DiagnosticStatus, summary: string, scope?: DiagnosticScope, evidence: Array<DiagnosticEvidence>, remediation?: string, remediationDetail?: DiagnosticRemediation, };
export type DiagnosticRun = { runId: string, startedAt: string, finishedAt: string, overallStatus: DiagnosticStatus, checks: Array<DiagnosticCheck>, };
export type StartupCohort = "cold" | "warm";
export type StartupOutcome = "ready" | "failed" | "cancelled" | "incomplete";
export type StartupPhase = "eligibility_prepare" | "runtime_spawn" | "runtime_initialize" | "mcp_endpoint_ready" | "tunnel_resolve" | "tunnel_launch" | "control_plane_connected" | "mcp_target_ready" | "harness_ready";
export type StartupPhaseStatus = "completed" | "failed" | "skipped" | "incomplete";
export type StartupPhaseTiming = { phase: StartupPhase, startedOffsetMs: number, durationMs: number, status: StartupPhaseStatus, safeErrorCode?: string, };
export type StartupAttempt = { attemptId: string, harnessSessionId: string, cohort: StartupCohort,
/**
 * RFC3339 UTC wall-clock timestamp for display and correlation only.
 */
startedAt: string, finishedAt?: string, outcome: StartupOutcome, totalDurationMs?: number, phases: Array<StartupPhaseTiming>, failureCode?: string, appVersion: string, runtimeVersion?: string, tunnelVersion?: string, };
