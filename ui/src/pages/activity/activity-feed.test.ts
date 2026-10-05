import { describe, expect, it } from "vitest";
import type { ActivityEntry } from "@/services/harness/control-read";
import {
  activityOutcomeKind,
  activityRows,
  filterActivityRows,
  reconcileActivitySelection,
} from "./activity-feed";

function entry(overrides: Partial<ActivityEntry> = {}): ActivityEntry {
  return {
    eventId: "event_01",
    requestId: "request_01",
    timestamp: "2026-10-05T00:00:00.000Z",
    category: "execution",
    eventType: "execution_completed",
    lifecycleState: "completed",
    toolName: "read_file",
    decisionSummary: "allow",
    matchedRuleIds: [],
    policyReason: null,
    outcomeSummary: "success",
    ...overrides,
  };
}

describe("M8 T7 activity presentation", () => {
  it("keeps policy denial, approval denial, execution failure, unavailable and pending distinct", () => {
    expect(activityOutcomeKind(entry({ outcomeSummary: "denied", decisionSummary: "deny", policyReason: "matched_user_rules" }))).toBe("policy_denied");
    expect(activityOutcomeKind(entry({ outcomeSummary: "denied", decisionSummary: "ask", policyReason: null, failureCategory: "approval_denied", approvalDecision: "deny" }))).toBe("approval_denied");
    expect(activityOutcomeKind(entry({ outcomeSummary: "failed", decisionSummary: "allow", failureCategory: "execution_failed" }))).toBe("execution_failed");
    expect(activityOutcomeKind(entry({ outcomeSummary: "failed", decisionSummary: "ask", failureCategory: "execution_failed" }))).toBe("failed");
    expect(activityOutcomeKind(entry({ outcomeSummary: "failed", failureCategory: "bash_unavailable" }))).toBe("unavailable");
    expect(activityOutcomeKind(entry({ outcomeSummary: "denied", policyReason: "tool_disabled", decisionSummary: "deny" }))).toBe("unavailable");
    expect(activityOutcomeKind(entry({ outcomeSummary: "denied", policyReason: "unsupported_capability", decisionSummary: "ask" }))).toBe("unavailable");
    expect(activityOutcomeKind(entry({ outcomeSummary: "failed", failureCategory: "bash_spawn_failed" }))).toBe("execution_failed");
    expect(activityOutcomeKind(entry({ outcomeSummary: "approval_required", lifecycleState: "awaiting_approval" }))).toBe("pending");
    expect(activityOutcomeKind(entry({ eventType: "approval_expired", outcomeSummary: "failed", failureCategory: undefined }))).toBe("expired");
    expect(activityOutcomeKind(entry({ eventType: "approval_cancelled", outcomeSummary: "failed", failureCategory: undefined }))).toBe("cancelled");
    expect(activityOutcomeKind(entry({ outcomeSummary: "denied", decisionSummary: null, policyReason: null }))).toBe("denied");
  });

  it("filters only the supplied bounded rows and keeps non-tool events readable", () => {
    const rows = activityRows([
      entry({ eventId: "success", toolName: "read_file" }),
      entry({ eventId: "failed", toolName: "bash", outcomeSummary: "failed", failureCategory: "execution_failed" }),
      entry({ eventId: "lifecycle", category: "workspace", eventType: "workspace_validated", toolName: null }),
    ], 1);
    expect(filterActivityRows(rows, "success").map((row) => row.entry.eventId)).toEqual(["success", "lifecycle"]);
    expect(filterActivityRows(rows, "attention").map((row) => row.entry.eventId)).toEqual(["failed"]);
    expect(filterActivityRows(rows, "tool:bash").map((row) => row.entry.eventId)).toEqual(["failed"]);
    expect(rows.find((row) => row.entry.eventId === "lifecycle")?.entry.toolName).toBeNull();
  });

  it("uses stable event IDs and snapshot-local identity for missing or duplicate IDs", () => {
    const initial = activityRows([entry({ eventId: "stable" }), entry({ eventId: undefined })], 1);
    expect(initial[0].rowKey).toBe("event:stable");
    expect(initial[1].snapshotLocal).toBe(true);
    const selectedStable = { rowKey: initial[0].rowKey, eventId: initial[0].stableEventId };
    const refreshed = activityRows([entry({ eventId: "other" }), entry({ eventId: "stable" })], 2);
    expect(reconcileActivitySelection(selectedStable, refreshed)).toEqual({ rowKey: "event:stable", eventId: "stable" });
    expect(reconcileActivitySelection({ rowKey: initial[1].rowKey }, refreshed)).toBeNull();

    const duplicated = activityRows([entry({ eventId: "same" }), entry({ eventId: "same" })], 3);
    expect(duplicated.every((row) => row.snapshotLocal)).toBe(true);
    expect(duplicated[0].rowKey).not.toBe(duplicated[1].rowKey);
  });
});
