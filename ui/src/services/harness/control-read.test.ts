import { describe, expect, it } from "vitest";
import { controlReadError, parseActivity, parseTools } from "./control-read";

describe("machine-only control read error boundary", () => {
  it.each(["RUNTIME_UNAVAILABLE", "RUNTIME_NOT_READY", "INVALID_REQUEST"])("preserves safe code %s without prose", (code) => {
    expect(controlReadError({ code, message: "secret" })).toEqual({ code });
  });
  it("drops unknown code, messages and thrown details", () => {
    expect(controlReadError({ code: "SECRET_CODE", message: "API_KEY" })).toEqual({ code: "CONTROL_READ_FAILED" });
    expect(controlReadError(new Error("SECRET"))).toEqual({ code: "CONTROL_READ_FAILED" });
  });
});

describe("control-read M5 projections", () => {
  it("preserves Runtime-owned Bash availability and readiness observations", () => {
    expect(parseTools([{
      name: "bash",
      declaredCapability: "developer_shell",
      availability: "unavailable_by_mode",
      decision: "deny",
      reason: "Developer/Test mode is required.",
      readiness: "unavailable",
      readinessReason: "Enable Developer/Test mode and restart Runtime.",
      rawCommand: "secret should be dropped",
    }])).toEqual([{
      name: "bash",
      declaredCapability: "developer_shell",
      availability: "unavailable_by_mode",
      decision: "deny",
      reason: "Developer/Test mode is required.",
      readiness: "unavailable",
      readinessReason: "Enable Developer/Test mode and restart Runtime.",
    }]);
  });

  it("accepts the full M4 event set and safe Bash evidence without argv/env/output", () => {
    const entries = [
      "request_received", "workspace_validated", "policy_evaluated", "execution_started",
      "execution_completed", "execution_failed", "request_denied", "approval_required",
      "approval_resolved", "approval_expired", "approval_cancelled", "session_grant_matched",
    ].map((eventType) => ({
      eventId: `event-${eventType}`,
      requestId: "request-1",
      timestamp: "2026-09-08T01:02:03.004Z",
      category: eventType.startsWith("approval") || eventType === "session_grant_matched" ? "approval" : "execution",
      eventType,
      lifecycleState: "executing",
      toolName: "bash",
      decisionSummary: "ask",
      matchedRuleIds: ["review-git"],
      policyReason: "matched_user_rules",
      outcomeSummary: "in_progress",
      failureCategory: eventType === "execution_failed" ? "bash_timeout" : undefined,
      bash: {
        mode: "developer_test",
        modeEligible: true,
        capability: "developer_shell",
        cwd: "active_workspace",
        commandPreview: "git status",
        classification: "classified",
        riskFlags: [],
        approvalRequestId: "approval-1",
        sessionGrantHit: eventType === "session_grant_matched",
        exitCode: eventType === "execution_completed" ? 0 : undefined,
        durationMs: eventType === "execution_completed" ? 12 : undefined,
        argv: ["must not cross the adapter boundary"],
        env: { SECRET: "must not cross the adapter boundary" },
        output: "must not cross the adapter boundary",
      },
    }));

    const parsed = parseActivity(entries);
    expect(parsed).toHaveLength(12);
    expect(parsed[0].eventType).toBe("request_received");
    expect(parsed[10].eventType).toBe("approval_cancelled");
    expect(parsed[11].bash).toMatchObject({
      mode: "developer_test",
      cwd: "active_workspace",
      commandPreview: "git status",
      sessionGrantHit: true,
    });
    expect(parsed[11].bash).not.toHaveProperty("argv");
    expect(parsed[11].bash).not.toHaveProperty("env");
    expect(parsed[11].bash).not.toHaveProperty("output");
  });

  it("keeps old M4 snapshots readable when new projection fields are omitted", () => {
    expect(parseActivity([{
      timestamp: null,
      eventType: "approval_required",
      toolName: "write",
      decisionSummary: "ask",
      outcomeSummary: "approval_required",
    }])).toEqual([{
      timestamp: null,
      eventType: "approval_required",
      toolName: "write",
      decisionSummary: "ask",
      matchedRuleIds: [],
      policyReason: null,
      outcomeSummary: "approval_required",
    }]);
  });

  it("uses the Runtime's 1027 UTF-8 byte preview bound", () => {
    const entry = {
      eventId: "event-preview",
      requestId: "request-preview",
      timestamp: "2026-09-08T01:02:03.004Z",
      category: "execution",
      eventType: "execution_started",
      lifecycleState: "executing",
      toolName: "bash",
      decisionSummary: "allow",
      outcomeSummary: "in_progress",
      bash: { mode: "developer_test", modeEligible: true, capability: "developer_shell", cwd: "active_workspace", commandPreview: "x".repeat(1024), sessionGrantHit: false },
    };
    expect(parseActivity([entry])[0].bash?.commandPreview).toHaveLength(1024);
    expect(() => parseActivity([{ ...entry, bash: { ...entry.bash, commandPreview: "x".repeat(1028) } }])).toThrow();
  });
});
