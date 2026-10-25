import { describe, expect, it } from "vitest";
import type { DiagnosticRun, StartupAttempt } from "@protocol/v1";
import { formatDiagnosticCopy } from "./diagnostic-copy";

const run: DiagnosticRun = {
  runId: "run-1",
  startedAt: "2026-10-25T00:00:00Z",
  finishedAt: "2026-10-25T00:00:01Z",
  overallStatus: "warning",
  checks: [{
    id: "workspace",
    status: "warning",
    summary: "safe summary",
    scope: "workspace",
    evidence: [{ key: "exists", value: "true" }, { key: "error_code", value: "C:\\private\\secret" }],
    remediation: { code: "workspace_choose", summary: "safe", target: "settings_workspace" },
  }],
};
const startup: StartupAttempt = {
  attemptId: "attempt-1",
  harnessSessionId: "session-1",
  cohort: "warm",
  startedAt: "2026-10-25T00:00:00Z",
  finishedAt: "2026-10-25T00:00:01Z",
  outcome: "ready",
  totalDurationMs: 100,
  phases: [{ phase: "harness_ready", startedOffsetMs: 90, durationMs: 10, status: "completed" }],
  appVersion: "0.1.3",
};

describe("formatDiagnosticCopy", () => {
  it("copies only allowlisted fields and redacts path-like sentinel values", () => {
    const output = formatDiagnosticCopy(run, startup);
    expect(output).toContain("overall=warning");
    expect(output).toContain("remediation=workspace_choose");
    expect(output).toContain("<redacted>");
    expect(output).not.toContain("C:\\private\\secret");
    expect(output).not.toContain("safe summary");
    expect(output).toContain("startup cohort=warm outcome=ready");
  });
});

