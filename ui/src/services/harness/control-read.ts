// Display-only projections of M8-C wire responses (protocol/src/v1/control_read.rs).
// No Runtime imports, config reads, policy evaluation or availability inference.
import type { PolicyDecision, PolicyReason, RuntimeReadiness } from "@protocol/v1";

export type ToolDiagnostic = {
  name: string;
  availability: "available" | "available_with_approval" | "unavailable_by_policy" | "unavailable_by_runtime" | "disabled";
  decision: PolicyDecision;
};
export type RuntimeDiagnostics = {
  schemaVersion: 1;
  capturedAt: string;
  runtime: { readiness: RuntimeReadiness };
  security: {
    workspaceValidation: "not_initialized" | "validated_at_initialization";
    policyEngineLoaded: boolean;
  };
  tools: { declaredCount: number; effectiveCount: number };
};
export type ActivityEntry = {
  timestamp: string | null;
  eventType: "request_received" | "workspace_validated" | "policy_evaluated" | "execution_started" | "execution_completed" | "execution_failed" | "request_denied" | "approval_required";
  toolName: string | null;
  decisionSummary: PolicyDecision | null;
  matchedRuleIds: string[];
  policyReason: PolicyReason | null;
  outcomeSummary: "in_progress" | "success" | "failed" | "denied" | "approval_required";
};
export interface ControlReadClient {
  runtimeGetDiagnostics(): Promise<RuntimeDiagnostics>;
  runtimeGetActivitySnapshot(limit: number): Promise<ActivityEntry[]>;
  runtimeGetEffectiveTools(): Promise<ToolDiagnostic[]>;
}

const invalid = () => { throw { code: "INTERNAL_ERROR" }; };
function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) return invalid();
  return value as Record<string, unknown>;
}
function enumeration<T extends string>(value: unknown, allowed: readonly T[]): T {
  return allowed.includes(value as T) ? value as T : invalid();
}
function count(value: unknown): number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0 ? value : invalid();
}
function boolean(value: unknown): boolean { return typeof value === "boolean" ? value : invalid(); }
function timestamp(value: unknown): string {
  // Never echo arbitrary error/path text in a time element or its attributes.
  return typeof value === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/.test(value) && Number.isFinite(Date.parse(value)) ? value : invalid();
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
export function parseDiagnostics(value: unknown): RuntimeDiagnostics {
  const data = record(value);
  if (data.schemaVersion !== 1) return invalid();
  const runtime = record(data.runtime), security = record(data.security), tools = record(data.tools);
  return {
    schemaVersion: 1, capturedAt: timestamp(data.capturedAt),
    runtime: { readiness: enumeration(runtime.readiness, ["not_ready", "initializing", "ready", "degraded"]) },
    security: {
      workspaceValidation: enumeration(security.workspaceValidation, ["not_initialized", "validated_at_initialization"]),
      policyEngineLoaded: boolean(security.policyEngineLoaded),
    },
    tools: { declaredCount: count(tools.declaredCount), effectiveCount: count(tools.effectiveCount) },
  };
}
export function parseTools(value: unknown): ToolDiagnostic[] {
  if (!Array.isArray(value)) return invalid();
  return value.map((item: unknown) => {
    const tool = record(item);
    return {
      name: toolName(tool.name),
      availability: enumeration(tool.availability, ["available", "available_with_approval", "unavailable_by_policy", "unavailable_by_runtime", "disabled"]),
      decision: enumeration(tool.decision, decisions),
    };
  });
}
export function parseActivity(value: unknown): ActivityEntry[] {
  if (!Array.isArray(value) || value.length > 256) return invalid();
  return value.map((item: unknown) => {
    const entry = record(item);
    return {
      timestamp: entry.timestamp === null ? null : timestamp(entry.timestamp),
      eventType: enumeration(entry.eventType, ["request_received", "workspace_validated", "policy_evaluated", "execution_started", "execution_completed", "execution_failed", "request_denied", "approval_required"]),
      toolName: entry.toolName === null ? null : toolName(entry.toolName),
      decisionSummary: entry.decisionSummary === null ? null : enumeration(entry.decisionSummary, decisions),
      matchedRuleIds: entry.matchedRuleIds === undefined ? [] : Array.isArray(entry.matchedRuleIds) ? entry.matchedRuleIds.map(ruleId) : invalid(),
      policyReason: entry.policyReason === undefined || entry.policyReason === null ? null : enumeration(entry.policyReason, policyReasons),
      outcomeSummary: enumeration(entry.outcomeSummary, ["in_progress", "success", "failed", "denied", "approval_required"]),
    };
  });
}

// Fixed copy only. Never render rejection.message, stack, JSON or unknown codes.
export function controlReadError(error: unknown): string {
  const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
  switch (code) {
    case "RUNTIME_UNAVAILABLE": return "Runtime unavailable. Start or reconnect Runtime, then refresh.";
    case "RUNTIME_NOT_READY": return "Runtime not ready. Wait for initialization, then refresh.";
    case "INVALID_REQUEST": return "Invalid control read request.";
    default: return "Control read failed. Refresh to try again.";
  }
}
