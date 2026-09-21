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
export type MachineErrorCode = "NOT_INITIALIZED" | "ALREADY_INITIALIZED" | "PROTOCOL_VERSION_MISMATCH" | "RUNTIME_NOT_READY" | "INVALID_ARGUMENT" | "APPROVAL_NOT_FOUND" | "APPROVAL_ALREADY_RESOLVED" | "INTERNAL_ERROR";
export type EmptyParams = Record<string, never>;
export type PolicyDecision = "deny" | "ask" | "allow";
export type AppConfig = { schemaVersion: number, configRevision: string, workspace: WorkspaceConfig, tunnel: TunnelConfig, security: SecurityConfig, ui: UiConfig, };
export type WorkspaceConfig = { rootPath: string, };
export type TunnelConfig = { tunnelId: string, controlPlaneHttpProxy?: string, tunnelClientPath?: string, autoDetect: boolean, apiKeyRef?: SecretReference, };
export type SecretReference = { provider: string, key: string, };
export type SecurityMode = "normal" | "developer_test";
export type SecurityConfig = { mode: SecurityMode, developerTestMode: boolean, };
export type UiConfig = { theme?: string, };
export type StartEligibilityStatus = "setup_required" | "configuration_invalid" | "runtime_distribution_invalid" | "tunnel_id_required" | "api_key_required" | "tunnel_distribution_invalid" | "ready_to_start" | "runtime_already_running" | "harness_already_running" | "lifecycle_conflict";
export type StartEligibility = { status: StartEligibilityStatus, canStart: boolean, message: string, };
export type RuntimeConfigSnapshot = { configRevision: string, workspace: WorkspaceConfig, security: SecurityConfig, runtimeLimits: RuntimeLimits, };
export type RuntimeLimits = { maxConcurrentRequests: number, maxActivityEvents: number, maxReadBytes: number, };
export type ClientInfo = { name: string, title: string, version: string, };
export type ServerInfo = { name: string, title: string, version: string, };
export type InitializeParams = { clientInfo: ClientInfo, protocolVersion: number, capabilities: { [key in string]?: boolean }, runtimeConfig: RuntimeConfigSnapshot, };
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
export type RiskLevel = "low" | "medium" | "high";
export type ToolDescriptor = { name: string, description: string, implemented: boolean, enabled: boolean, exposed: boolean, effectivePolicy: PolicyDecision, scope: string, readiness: ToolReadiness, riskLevel: RiskLevel, };
export type ToolsListResult = { items: Array<ToolDescriptor>, };
export type PolicySource = "system" | "user";
export type PolicyRule = { id: string, name: string, decision: PolicyDecision, scope: string, reason: string, source: PolicySource, editable: boolean, enabled: boolean, };
export type PoliciesListResult = { items: Array<PolicyRule>, };
export type ActivityEventType = "runtime_started" | "runtime_stopped" | "tool_requested" | "validation_failed" | "workspace_violation" | "policy_evaluated" | "approval_requested" | "approval_resolved" | "execution_started" | "execution_completed" | "execution_failed" | "execution_timed_out" | "execution_cancelled";
export type ActivityEvent = { eventId: string, 
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
export type ApprovalStatus = "pending" | "approved" | "denied" | "expired" | "cancelled";
export type ApprovalDecision = "allow_once" | "deny";
export type ApprovalRequest = { requestId: string, tool: string, sanitizedArguments: { [key in string]?: string }, createdAt: string, expiresAt: string, status: ApprovalStatus, resolvedAt?: string, decision?: ApprovalDecision, };
export type ApprovalsListParams = { status?: ApprovalStatus, };
export type ApprovalsListResult = { items: Array<ApprovalRequest>, };
export type ApprovalsResolveParams = { requestId: string, decision: ApprovalDecision, };
export type ApprovalsResolveResult = { approval: ApprovalRequest, };
export type DiagnosticStatus = "pass" | "warning" | "fail";
export type DiagnosticCheck = { id: string, status: DiagnosticStatus, summary: string, remediation?: string, };
export type DiagnosticRun = { runId: string, startedAt: string, finishedAt: string, overallStatus: DiagnosticStatus, checks: Array<DiagnosticCheck>, };
