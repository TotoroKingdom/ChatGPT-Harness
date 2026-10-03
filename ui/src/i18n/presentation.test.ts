import { describe, expect, it } from "vitest";
import type {
  ActivityEventType as ProtocolActivityEventType,
  ApprovalDecision,
  ApprovalStatus,
  BashClassification,
  BashRiskFlag,
  PolicyDecision,
  PolicyReason,
  RiskLevel,
  SecurityMode,
  ToolReadiness,
} from "@protocol/v1";
import type {
  ActivityCategory,
  ActivityEventType as ControlReadActivityEventType,
  ActivityFailureCategory,
  ActivityLifecycleState,
  ActivityOutcome,
  ToolAvailability,
} from "@/services/harness/control-read";
import { i18n } from "@/i18n";
import {
  activityCategoryKey,
  activityEventKey,
  activityFailureCategoryKey,
  activityLifecycleStateKey,
  activityOutcomeKey,
  approvalDecisionKey,
  approvalStatusKey,
  bashClassificationKey,
  bashRiskFlagKey,
  componentStatusKey,
  errorCodeKey,
  overallStatusKey,
  pageEyebrowKey,
  pageLabelKey,
  policyDecisionKey,
  policySnapshotStatusKey,
  policyReasonKey,
  policySourceKey,
  presentError,
  riskLevelKey,
  runtimeReadinessKey,
  securityModeKey,
  startEligibilityKey,
  systemPolicyKeys,
  toolAvailabilityKey,
  toolReadinessKey,
} from "./presentation";

function translated(key: string): void {
  expect(i18n.t(key)).not.toBe(key);
  expect(i18n.t(key)).not.toBe("");
}

describe("typed machine-value presentation mappings", () => {
  it("maps every shared status and security family to a translation key", () => {
    for (const status of ["stopped", "starting", "ready", "degraded", "error", "stopping"] as const) {
      translated(componentStatusKey(status));
      translated(overallStatusKey(status));
    }
    for (const mode of ["normal", "developer_test"] as SecurityMode[]) translated(securityModeKey(mode));
    for (const readiness of ["not_ready", "initializing", "ready", "degraded"] as const) translated(runtimeReadinessKey(readiness));
    for (const eligibility of ["setup_required", "configuration_invalid", "runtime_distribution_invalid", "tunnel_id_required", "api_key_required", "tunnel_distribution_invalid", "ready_to_start", "runtime_already_running", "harness_already_running", "lifecycle_conflict"] as const) translated(startEligibilityKey(eligibility));
  });

  it("maps policy, availability, readiness and risk families", () => {
    for (const decision of ["allow", "ask", "deny"] as PolicyDecision[]) translated(policyDecisionKey(decision));
    for (const reason of ["system_guardrail", "matched_user_rules", "tool_default", "invalid_policy_snapshot", "invalid_context", "unsupported_capability", "tool_disabled"] as PolicyReason[]) translated(policyReasonKey(reason));
    for (const availability of ["available", "available_with_approval", "unavailable_by_mode", "unavailable_by_policy", "unavailable_by_runtime", "disabled"] as ToolAvailability[]) translated(toolAvailabilityKey(availability));
    for (const readiness of ["ready", "unavailable"] as ToolReadiness[]) translated(toolReadinessKey(readiness));
    for (const risk of ["low", "medium", "high"] as RiskLevel[]) translated(riskLevelKey(risk));
    for (const status of ["valid", "invalid"] as const) translated(policySnapshotStatusKey(status));
    for (const source of ["system", "user"] as const) translated(policySourceKey(source));
  });

  it("maps approval, activity and Bash families", () => {
    for (const status of ["pending", "approved", "denied", "expired", "cancelled"] as ApprovalStatus[]) translated(approvalStatusKey(status));
    for (const decision of ["allow_once", "allow_for_session", "deny"] as ApprovalDecision[]) translated(approvalDecisionKey(decision));
    for (const category of ["request", "workspace", "policy", "execution", "approval"] as ActivityCategory[]) translated(activityCategoryKey(category));
    for (const event of ["runtime_started", "runtime_stopped", "tool_requested", "validation_failed", "workspace_violation", "policy_evaluated", "approval_requested", "approval_resolved", "approval_denied", "approval_expired", "approval_cancelled", "execution_started", "execution_completed", "execution_failed", "execution_timed_out", "execution_cancelled"] as ProtocolActivityEventType[]) translated(activityEventKey(event));
    for (const event of ["request_received", "workspace_validated", "policy_evaluated", "execution_started", "execution_completed", "execution_failed", "request_denied", "approval_required", "approval_resolved", "approval_expired", "approval_cancelled", "session_grant_matched"] as ControlReadActivityEventType[]) translated(activityEventKey(event));
    for (const lifecycle of ["received", "validating", "workspace_validating", "policy_evaluating", "denied", "awaiting_approval", "executing", "completed", "failed"] as ActivityLifecycleState[]) translated(activityLifecycleStateKey(lifecycle));
    for (const outcome of ["in_progress", "success", "failed", "denied", "approval_required"] as ActivityOutcome[]) translated(activityOutcomeKey(outcome));
    for (const failure of ["tool_disabled", "target_changed", "atomic_failure", "io_failure", "workspace_violation", "invalid_arguments", "execution_failed", "bash_unavailable", "bash_invalid_command", "bash_command_too_long", "bash_timeout", "bash_output_limit", "bash_concurrency_limit", "bash_spawn_failed", "bash_cancelled", "bash_process_failed"] as ActivityFailureCategory[]) translated(activityFailureCategoryKey(failure));
    for (const classification of ["classified", "unclassified_high_risk"] as BashClassification[]) translated(bashClassificationKey(classification));
    for (const flag of ["command_substitution", "backticks", "eval", "nested_shell", "interpreter_inline_code", "redirection", "dynamic_expansion", "malformed_quoting", "unsupported_grammar"] as BashRiskFlag[]) translated(bashRiskFlagKey(flag));
  });

  it("keeps route identity in keys while translating labels", async () => {
    for (const page of ["overview", "tools", "policies", "activity", "approvals", "settings"] as const) {
      translated(pageLabelKey(page));
      translated(pageEyebrowKey(page));
    }
    await i18n.changeLanguage("zh-CN");
    expect(i18n.t(pageLabelKey("overview"))).toBe("概览");
    await i18n.changeLanguage("en");
    expect(i18n.t(pageLabelKey("overview"))).toBe("Overview");
  });
});

describe("error and system-policy presentation", () => {
  it("localizes the real Supervisor RPC envelope while retaining the raw code", () => {
    for (const code of ["APPROVAL_NOT_FOUND", "APPROVAL_ALREADY_RESOLVED", "RUNTIME_NOT_READY", "BASH_TIMEOUT"]) {
      expect(errorCodeKey(`RPC_${code}`)).toBe(errorCodeKey(code));
      expect(presentError({ code: `RPC_${code}` }).code).toBe(`RPC_${code}`);
      translated(errorCodeKey(`RPC_${code}`));
    }
    expect(errorCodeKey("RPC_UNKNOWN_FAILURE")).toBe("errors.unknown");
    expect(presentError("bad code").code).toBeUndefined();
  });
  it("localizes known protocol, host and control-read errors", () => {
    for (const code of ["RUNTIME_NOT_READY", "BASH_TIMEOUT", "CONFIG_INVALID", "RUNTIME_UNAVAILABLE", "INVALID_REQUEST", "CONTROL_READ_FAILED"] as const) {
      translated(errorCodeKey(code));
    }
    expect(errorCodeKey("NOT_A_REAL_SUCCESS_CODE")).toBe("errors.unknown");
    expect(errorCodeKey("NOT_A_REAL_SUCCESS_CODE")).not.toContain("allow");
  });

  it("only exposes explicitly sanitized fallback detail", () => {
    expect(presentError({ code: "INTERNAL_ERROR", message: "secret" })).toEqual({
      summaryKey: "errors.codes.internalError",
      code: "INTERNAL_ERROR",
    });
    expect(presentError({ code: "UNKNOWN_CODE", message: "safe diagnostic" }, "safe diagnostic")).toEqual({
      summaryKey: "errors.unknown",
      code: "UNKNOWN_CODE",
      detail: "safe diagnostic",
    });
    expect(presentError({ code: "bad code", message: "secret" }, "safe diagnostic").code).toBeUndefined();
  });

  it("provides known system-rule friendly keys without rewriting raw identity", () => {
    const rawUserText = "User-authored reason stays verbatim.";
    for (const rawId of ["system-read-workspace", "system-deny-outside"] as const) {
      const keys = systemPolicyKeys(rawId);
      translated(keys.name);
      translated(keys.reason);
      expect(rawId).toMatch(/^system-/);
    }
    expect(rawUserText).toBe("User-authored reason stays verbatim.");
    expect(systemPolicyKeys("user-custom-rule").name).toBe("policy.system.unknownName");
  });
});
