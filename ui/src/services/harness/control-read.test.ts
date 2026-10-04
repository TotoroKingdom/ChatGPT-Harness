import { describe, expect, it } from "vitest";
import { controlReadError, parseActivity, parseTools, parseDiagnostics } from "./control-read";

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

describe("M7 typed evidence boundary", () => {
  const entry = { eventType: "execution_completed", timestamp: null, toolName: "write", decisionSummary: "ask", outcomeSummary: "success", toolEnabled: true, capability: "workspace_write", approvalId: "request-123", approvalDecision: "allow_once", durationMs: 5, sanitizedArguments: { tool: "write", logicalPath: "safe.txt", content: "CONTENT_SENTINEL" }, resultEvidence: { tool: "write", bytesWritten: 7, disposition: "created", content: "RESULT_SENTINEL" } };
  it("retains safe scalars and identities while dropping payload fields", () => {
    const parsed = parseActivity([entry])[0];
    expect(parsed.approvalId).toBe("request-123"); expect(parsed.durationMs).toBe(5);
    expect(parsed.sanitizedArguments).toEqual({ tool: "write", logicalPath: "safe.txt" });
    expect(JSON.stringify(parsed)).not.toContain("SENTINEL");
    expect(parsed.resultEvidence).toEqual({ tool: "write", bytesWritten: 7, disposition: "created" });
  });
  it.each(["../private", "C:/private", "/private", "a\u0000b"])("rejects unsafe logical path %s", (logicalPath) => {
    expect(() => parseActivity([{ ...entry, sanitizedArguments: { tool: "write", logicalPath } }])).toThrow();
  });
  it("rejects oversized evidence and invalid duration/result scalars", () => {
    expect(() => parseActivity([{ ...entry, sanitizedArguments: { tool: "glob", pattern: "x".repeat(1100) } }])).toThrow();
    expect(() => parseActivity([{ ...entry, durationMs: -1 }])).toThrow();
    expect(() => parseActivity([{ ...entry, resultEvidence: { tool: "read_file", bytesRead: -1 } }])).toThrow();
  });
});

describe("M7 typed reason and counter observations", () => {
  it.each(["approval_required", "tool_disabled", "developer_mode_required", "bash_executable_unavailable", "policy_denied", "invalid_policy_snapshot", "runtime_not_ready"])("retains reason %s independently from prose", (statusReason) => {
    expect(parseTools([{ name: "write", availability: "unavailable_by_runtime", decision: "deny", statusReason, reason: "SECRET" }])[0].statusReason).toBe(statusReason);
    expect(() => parseTools([{ name: "write", availability: "available", decision: "allow", statusReason: "SECRET" }])).toThrow();
  });
  it("retains real counters and safe last code while discarding raw detail", () => {
    const parsed = parseDiagnostics({ schemaVersion: 1, capturedAt: "2026-10-04T01:00:00Z", runtime: { readiness: "ready", configRevision: "47" }, security: { workspaceValidation: "validated_at_initialization", policyEngineLoaded: true }, tools: { declaredCount: 5, effectiveCount: 4 }, requests: { observation: "instrumented", activeRequests: 3, pendingApprovals: 2, runningExecutions: 1 }, lastError: { code: "BASH_TIMEOUT", message: "SECRET" } });
    expect(parsed.requests).toEqual({ activeRequests: 3, pendingApprovals: 2, runningExecutions: 1 });
    expect(parsed.runtime.configRevision).toBe("47"); expect(parsed.lastError?.code).toBe("BASH_TIMEOUT"); expect(JSON.stringify(parsed)).not.toContain("SECRET");
  });
});
