// Display-only projections of the local Runtime control-read responses
// (protocol/src/v1/control_read.rs). These parsers deliberately do not
// evaluate policy, read persisted config, or infer Runtime readiness from a
// desired path or mode.
import type { ToolStatusReason, SanitizedArgumentsEvidence, ExecutionResultEvidence, ToolCapability as SharedToolCapability, AuditEventType as SharedAuditEventType, AuditLifecycleState as SharedAuditLifecycleState, AuditOutcome as SharedAuditOutcome, ActivityCategory as SharedActivityCategory, ActivityFailureCategory as SharedActivityFailureCategory, ApprovalDecision, PolicyDecision, PolicyReason, RuntimeReadiness } from "@protocol/v1";

export type ToolCapability = SharedToolCapability;
export type ToolAvailability =
  | "available"
  | "available_with_approval"
  | "unavailable_by_mode"
  | "unavailable_by_policy"
  | "unavailable_by_runtime"
  | "disabled";
export type ToolReadiness = "ready" | "unavailable";

export type ToolDiagnostic = {
  name: string;
  statusReason?: ToolStatusReason;
  availability: ToolAvailability;
  decision: PolicyDecision;
  // M4 read responses omitted these fields, so they remain optional at this
  // adapter boundary. Current Runtime responses supply them for Bash.
  declaredCapability?: ToolCapability | null;
  reason?: string;
  readiness?: ToolReadiness;
  readinessReason?: string;
};

export type RuntimeDiagnostics = {
  schemaVersion: 1;
  capturedAt: string;
  runtime: { readiness: RuntimeReadiness; configRevision?: string | null };
  requests?: { activeRequests: number; pendingApprovals: number; runningExecutions: number };
  lastError?: { code: string | null };
  security: {
    workspaceValidation: "not_initialized" | "validated_at_initialization";
    policyEngineLoaded: boolean;
  };
  tools: { declaredCount: number; effectiveCount: number };
};

export type BashClassification = "classified" | "unclassified_high_risk";
export type BashRiskFlag =
  | "command_substitution"
  | "backticks"
  | "eval"
  | "nested_shell"
  | "interpreter_inline_code"
  | "redirection"
  | "dynamic_expansion"
  | "malformed_quoting"
  | "unsupported_grammar";
export type BashWorkingDirectory = "active_workspace";

export type BashActivityEvidence = {
  mode: "normal" | "developer_test";
  modeEligible: boolean;
  capability: ToolCapability;
  cwd: BashWorkingDirectory;
  commandPreview?: string;
  classification?: BashClassification;
  riskFlags: BashRiskFlag[];
  approvalRequestId?: string;
  sessionGrantHit: boolean;
  exitCode?: number;
  durationMs?: number;
};

export type ActivityFailureCategory = SharedActivityFailureCategory;

export type ActivityCategory = SharedActivityCategory;
export type ActivityEventType = SharedAuditEventType;
export type ActivityLifecycleState = SharedAuditLifecycleState;
export type ActivityOutcome = SharedAuditOutcome;

export type ActivityEntry = {
  // New control-read responses carry IDs/category/lifecycle. They are optional
  // so M4 snapshots that predate this projection remain readable.
  eventId?: string;
  requestId?: string;
  category?: ActivityCategory;
  lifecycleState?: ActivityLifecycleState;
  timestamp: string | null;
  eventType: ActivityEventType;
  toolName: string | null;
  decisionSummary: PolicyDecision | null;
  matchedRuleIds: string[];
  policyReason: PolicyReason | null;
  outcomeSummary: ActivityOutcome;
  failureCategory?: ActivityFailureCategory;
  bash?: BashActivityEvidence;
  toolEnabled?: boolean;
  capability?: ToolCapability;
  sanitizedArguments?: SanitizedArgumentsEvidence;
  approvalId?: string;
  approvalDecision?: ApprovalDecision;
  durationMs?: number;
  resultEvidence?: ExecutionResultEvidence;
};

export interface ControlReadClient {
  runtimeGetDiagnostics(): Promise<RuntimeDiagnostics>;
  runtimeGetActivitySnapshot(limit: number): Promise<ActivityEntry[]>;
  runtimeGetEffectiveTools(): Promise<ToolDiagnostic[]>;
}

const invalid = (): never => { throw { code: "INTERNAL_ERROR" }; };
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return invalid();
  return value as Record<string, unknown>;
}
function enumeration<T extends string>(value: unknown, allowed: readonly T[]): T {
  return allowed.includes(value as T) ? value as T : invalid();
}
function optionalEnumeration<T extends string>(value: unknown, allowed: readonly T[]): T | undefined {
  if (value === undefined || value === null) return undefined;
  return enumeration(value, allowed);
}
function count(value: unknown): number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : invalid();
}
function integer(value: unknown): number {
  return typeof value === "number" && Number.isSafeInteger(value) ? value : invalid();
}
function boolean(value: unknown): boolean { return typeof value === "boolean" ? value : invalid(); }
function boundedText(value: unknown, maxBytes = 1024): string {
  if (typeof value !== "string" || value.length > maxBytes || new TextEncoder().encode(value).byteLength > maxBytes || /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/.test(value)) return invalid();
  return value;
}
function optionalText(value: unknown, max = 1024): string | undefined {
  if (value === undefined || value === null) return undefined;
  return boundedText(value, max);
}
function timestamp(value: unknown): string {
  // Never echo arbitrary error/path text in a time element or its attributes.
  return typeof value === "string" && value.length <= 64 && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value) && Number.isFinite(Date.parse(value)) ? value : invalid();
}
function safeIdentifier(value: unknown, max = 128): string {
  return typeof value === "string" && value.length > 0 && value.length <= max && /^[a-zA-Z0-9][a-zA-Z0-9_-]*$/.test(value) ? value : invalid();
}
function toolName(value: unknown): string {
  // Syntax check only, NOT a second registry. Names are source-controlled by Runtime.
  return typeof value === "string" && /^[a-zA-Z_][a-zA-Z0-9_]{0,63}$/.test(value) ? value : invalid();
}
function ruleId(value: unknown): string {
  return typeof value === "string" && /^[a-z0-9][a-zA-Z0-9_-]{0,63}$/.test(value) ? value : invalid();
}

const decisions = ["allow", "ask", "deny"] as const;
const policyReasons = ["system_guardrail", "matched_user_rules", "tool_default", "invalid_policy_snapshot", "invalid_context", "unsupported_capability", "tool_disabled"] as const;
const capabilities = ["workspace_read", "workspace_write", "developer_shell"] as const;
const statusReasons = ["available", "approval_required", "tool_disabled", "developer_mode_required", "runtime_not_ready", "capability_unavailable", "bash_executable_unavailable", "policy_denied", "invalid_policy_snapshot", "invalid_policy_context", "unsupported_capability", "metadata_invalid"] as const;
const availability = ["available", "available_with_approval", "unavailable_by_mode", "unavailable_by_policy", "unavailable_by_runtime", "disabled"] as const;
const readiness = ["ready", "unavailable"] as const;
const failureCategories = ["approval_denied", "approval_expired", "approval_cancelled", "tool_disabled", "target_changed", "atomic_failure", "io_failure", "workspace_violation", "invalid_arguments", "execution_failed", "bash_unavailable", "bash_invalid_command", "bash_command_too_long", "bash_timeout", "bash_output_limit", "bash_concurrency_limit", "bash_spawn_failed", "bash_cancelled", "bash_process_failed"] as const;
const activityCategories = ["request", "workspace", "policy", "execution", "approval"] as const;
const activityEventTypes = ["request_received", "workspace_validated", "policy_evaluated", "execution_started", "execution_completed", "execution_failed", "request_denied", "approval_required", "approval_resolved", "approval_expired", "approval_cancelled", "session_grant_matched"] as const;
const lifecycleStates = ["received", "validating", "workspace_validating", "policy_evaluating", "denied", "awaiting_approval", "executing", "completed", "failed"] as const;
const outcomes = ["in_progress", "success", "failed", "denied", "approval_required"] as const;
const modes = ["normal", "developer_test"] as const;
const riskFlags = ["command_substitution", "backticks", "eval", "nested_shell", "interpreter_inline_code", "redirection", "dynamic_expansion", "malformed_quoting", "unsupported_grammar"] as const;

function parseBashEvidence(value: unknown): BashActivityEvidence {
  const bash = record(value);
  const evidence: BashActivityEvidence = {
    mode: enumeration(bash.mode, modes),
    modeEligible: boolean(bash.modeEligible),
    capability: enumeration(bash.capability, capabilities),
    cwd: enumeration(bash.cwd, ["active_workspace"] as const),
    riskFlags: bash.riskFlags === undefined ? [] : Array.isArray(bash.riskFlags) ? bash.riskFlags.map((flag) => enumeration(flag, riskFlags)) : invalid(),
    sessionGrantHit: boolean(bash.sessionGrantHit),
  };
  // Runtime truncates this display projection at 1027 UTF-8 bytes. Preserve
  // the complete legal wire value rather than applying a smaller UI limit.
  const commandPreview = optionalText(bash.commandPreview, 1027);
  const approvalRequestId = bash.approvalRequestId === undefined || bash.approvalRequestId === null ? undefined : safeIdentifier(bash.approvalRequestId);
  const classification = optionalEnumeration(bash.classification, ["classified", "unclassified_high_risk"] as const);
  const exitCode = bash.exitCode === undefined || bash.exitCode === null ? undefined : integer(bash.exitCode);
  const durationMs = bash.durationMs === undefined || bash.durationMs === null ? undefined : count(bash.durationMs);
  if (commandPreview !== undefined) evidence.commandPreview = commandPreview;
  if (approvalRequestId !== undefined) evidence.approvalRequestId = approvalRequestId;
  if (classification !== undefined) evidence.classification = classification;
  if (exitCode !== undefined) evidence.exitCode = exitCode;
  if (durationMs !== undefined) evidence.durationMs = durationMs;
  return evidence;
}

export function parseDiagnostics(value: unknown): RuntimeDiagnostics {
  const data = record(value);
  if (data.schemaVersion !== 1) return invalid();
  const runtime = record(data.runtime), security = record(data.security), tools = record(data.tools);
  const parsed: RuntimeDiagnostics = {
    schemaVersion: 1, capturedAt: timestamp(data.capturedAt),
    runtime: { readiness: enumeration(runtime.readiness, ["not_ready", "initializing", "ready", "degraded"]) },
    security: {
      workspaceValidation: enumeration(security.workspaceValidation, ["not_initialized", "validated_at_initialization"]),
      policyEngineLoaded: boolean(security.policyEngineLoaded),
    },
    tools: { declaredCount: count(tools.declaredCount), effectiveCount: count(tools.effectiveCount) },
  };
  if (runtime.configRevision !== undefined && runtime.configRevision !== null) {
    if (typeof runtime.configRevision !== "string" || !/^(0|[1-9][0-9]*)$/.test(runtime.configRevision)) return invalid();
    parsed.runtime.configRevision = runtime.configRevision;
  }
  if (data.requests !== undefined) {
    const requests = record(data.requests);
    // Old responses may be explicitly not instrumented; never invent zero.
    if (requests.observation === "instrumented") parsed.requests = { activeRequests: count(requests.activeRequests), pendingApprovals: count(requests.pendingApprovals), runningExecutions: count(requests.runningExecutions) };
  }
  if (data.lastError !== undefined) {
    const error = record(data.lastError);
    const safeCodes = ["NOT_INITIALIZED", "ALREADY_INITIALIZED", "PROTOCOL_VERSION_MISMATCH", "RUNTIME_NOT_READY", "INVALID_ARGUMENT", "APPROVAL_NOT_FOUND", "APPROVAL_ALREADY_RESOLVED", "APPROVAL_SESSION_GRANT_UNAVAILABLE", "BASH_UNAVAILABLE", "BASH_INVALID_COMMAND", "BASH_COMMAND_TOO_LONG", "BASH_TIMEOUT", "BASH_OUTPUT_LIMIT", "BASH_CONCURRENCY_LIMIT", "BASH_SPAWN_FAILED", "BASH_CANCELLED", "BASH_PROCESS_FAILED", "INTERNAL_ERROR"];
    parsed.lastError = { code: typeof error.code === "string" && safeCodes.includes(error.code) ? error.code : null };
  }
  return parsed;
}

export function parseTools(value: unknown): ToolDiagnostic[] {
  if (!Array.isArray(value)) return invalid();
  return value.map((item: unknown) => {
    const tool = record(item);
    const name = toolName(tool.name);
    const parsed: ToolDiagnostic = {
      name,
      availability: enumeration(tool.availability, availability),
      decision: enumeration(tool.decision, decisions),
    };
    if (tool.statusReason !== undefined && tool.statusReason !== null) parsed.statusReason = enumeration(tool.statusReason, statusReasons);
    if (tool.declaredCapability !== undefined && tool.declaredCapability !== null) {
      parsed.declaredCapability = enumeration(tool.declaredCapability, capabilities);
    } else if (tool.declaredCapability === null) {
      parsed.declaredCapability = null;
    }
    // Preserve only Runtime-supplied observations. In particular, do not
    // synthesize readiness from desired config or an executable path.
    const parsedReadiness = optionalEnumeration(tool.readiness, readiness);
    // Runtime's additional remediation fields are the Bash diagnostic
    // projection. Do not turn an arbitrary legacy reason field on another
    // tool into UI evidence.
    const parsedReason = name === "bash" ? optionalText(tool.reason) : undefined;
    const parsedReadinessReason = name === "bash" ? optionalText(tool.readinessReason) : undefined;
    if (parsedReadiness !== undefined) parsed.readiness = parsedReadiness;
    if (parsedReason !== undefined) parsed.reason = parsedReason;
    if (parsedReadinessReason !== undefined) parsed.readinessReason = parsedReadinessReason;
    return parsed;
  });
}

function parseArguments(value: unknown): SanitizedArgumentsEvidence {
  const args = record(value);
  const tool = enumeration(args.tool, ["read_file", "glob", "grep", "write", "edit", "bash"] as const);
  if (tool === "glob") return { tool, pattern: boundedText(args.pattern, 1027) };
  if (tool === "grep") {
    return {
      tool,
      mode: enumeration(args.mode, ["literal", "regex"] as const),
      logicalScope: boundedText(args.logicalScope),
      queryBytes: count(args.queryBytes),
      includeCount: count(args.includeCount),
      excludeCount: count(args.excludeCount),
      maxResults: count(args.maxResults),
      maxOutputBytes: count(args.maxOutputBytes),
    };
  }
  if (tool === "bash") return { tool, commandPreview: boundedText(args.commandPreview, 1027) };
  const logicalPath = boundedText(args.logicalPath);
  if (!logicalPath || logicalPath.startsWith("/") || logicalPath.includes("\\") || logicalPath.includes(":") || logicalPath.split("/").includes("..")) return invalid();
  return { tool, logicalPath };
}
function parseResult(value: unknown): ExecutionResultEvidence {
  const result = record(value);
  const tool = enumeration(result.tool, ["read_file", "glob", "grep", "write", "edit", "bash"] as const);
  switch (tool) {
    case "read_file": return { tool, bytesRead: count(result.bytesRead) };
    case "glob": return { tool, matchCount: count(result.matchCount) };
    case "grep": return {
      tool,
      matchCount: count(result.matchCount),
      scannedFiles: count(result.scannedFiles),
      scannedBytes: count(result.scannedBytes),
      skippedFiles: count(result.skippedFiles),
      truncated: Boolean(result.truncated),
    };
    case "write": return { tool, bytesWritten: count(result.bytesWritten), disposition: enumeration(result.disposition, ["created", "overwritten"] as const) };
    case "edit": return { tool, replacements: count(result.replacements), bytesWritten: count(result.bytesWritten) };
    case "bash": return { tool, exitCode: integer(result.exitCode) };
  }
}

export function parseActivity(value: unknown): ActivityEntry[] {
  if (!Array.isArray(value) || value.length > 256) return invalid();
  return value.map((item: unknown) => {
    const entry = record(item);
    const parsed: ActivityEntry = {
      timestamp: entry.timestamp === undefined || entry.timestamp === null ? null : timestamp(entry.timestamp),
      eventType: enumeration(entry.eventType, activityEventTypes),
      toolName: entry.toolName === undefined || entry.toolName === null ? null : toolName(entry.toolName),
      decisionSummary: entry.decisionSummary === undefined || entry.decisionSummary === null ? null : enumeration(entry.decisionSummary, decisions),
      matchedRuleIds: entry.matchedRuleIds === undefined ? [] : Array.isArray(entry.matchedRuleIds) ? entry.matchedRuleIds.map(ruleId) : invalid(),
      policyReason: entry.policyReason === undefined || entry.policyReason === null ? null : enumeration(entry.policyReason, policyReasons),
      outcomeSummary: enumeration(entry.outcomeSummary, outcomes),
    };
    const eventId = entry.eventId === undefined || entry.eventId === null ? undefined : safeIdentifier(entry.eventId);
    const requestId = entry.requestId === undefined || entry.requestId === null ? undefined : safeIdentifier(entry.requestId);
    const category = optionalEnumeration(entry.category, activityCategories);
    const lifecycleState = optionalEnumeration(entry.lifecycleState, lifecycleStates);
    const failureCategory = optionalEnumeration(entry.failureCategory, failureCategories);
    if (eventId !== undefined) parsed.eventId = eventId;
    if (requestId !== undefined) parsed.requestId = requestId;
    if (category !== undefined) parsed.category = category;
    if (lifecycleState !== undefined) parsed.lifecycleState = lifecycleState;
    if (failureCategory !== undefined) parsed.failureCategory = failureCategory;
    if (entry.toolEnabled !== undefined && entry.toolEnabled !== null) parsed.toolEnabled = boolean(entry.toolEnabled);
    if (entry.capability !== undefined && entry.capability !== null) parsed.capability = enumeration(entry.capability, capabilities);
    if (entry.sanitizedArguments !== undefined && entry.sanitizedArguments !== null) parsed.sanitizedArguments = parseArguments(entry.sanitizedArguments);
    if (entry.approvalId !== undefined && entry.approvalId !== null) parsed.approvalId = safeIdentifier(entry.approvalId);
    if (entry.approvalDecision !== undefined && entry.approvalDecision !== null) parsed.approvalDecision = enumeration(entry.approvalDecision, ["allow_once", "allow_for_session", "deny"] as const);
    if (entry.durationMs !== undefined && entry.durationMs !== null) parsed.durationMs = count(entry.durationMs);
    if (entry.resultEvidence !== undefined && entry.resultEvidence !== null) parsed.resultEvidence = parseResult(entry.resultEvidence);
    if (entry.bash !== undefined && entry.bash !== null) parsed.bash = parseBashEvidence(entry.bash);
    return parsed;
  });
}

// Keep machine meaning only. Never echo rejection.message/stack/JSON here:
// this read boundary has deliberately stricter redaction than general errors.
export type ControlReadError = { code: "RUNTIME_UNAVAILABLE" | "RUNTIME_NOT_READY" | "INVALID_REQUEST" | "CONTROL_READ_FAILED" };
export function controlReadError(error: unknown): ControlReadError {
  const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
  switch (code) {
    case "RUNTIME_UNAVAILABLE":
    case "RUNTIME_NOT_READY":
    case "INVALID_REQUEST": return { code };
    default: return { code: "CONTROL_READ_FAILED" };
  }
}
