import type { ActivityEntry } from "@/services/harness/control-read";

/**
 * A presentation classification only.  The Runtime-provided outcome,
 * decision, failure category and approval decision are carried through as
 * evidence; this helper never evaluates policy or approval eligibility.
 */
export type ActivityOutcomeKind =
  | "success"
  | "policy_denied"
  | "approval_denied"
  | "execution_failed"
  | "failed"
  | "unavailable"
  | "pending"
  | "cancelled"
  | "expired"
  | "denied"
  | "in_progress";

export type ActivityFilter = "all" | "success" | "attention" | `tool:${string}`;

export type ActivityRow = {
  entry: ActivityEntry;
  /** Stable only when the Runtime supplied a unique eventId in this snapshot. */
  rowKey: string;
  stableEventId?: string;
  snapshotLocal: boolean;
};

export type ActivitySelection = {
  rowKey: string;
  eventId?: string;
};

const unavailableFailures = new Set([
  "tool_disabled",
  "bash_unavailable",
]);

const unavailableReasons = new Set(["tool_disabled", "unsupported_capability"]);

/**
 * Map reported fields to a user-facing outcome family.  The order matters:
 * an explicit approval denial or cancellation must not be presented as an
 * execution failure, and a denied policy decision must not be presented as a
 * generic Runtime error.
 */
export function activityOutcomeKind(entry: ActivityEntry): ActivityOutcomeKind {
  if (entry.eventType === "approval_expired" || entry.failureCategory === "approval_expired") return "expired";
  if (
    entry.eventType === "approval_cancelled"
    || entry.failureCategory === "approval_cancelled"
    || entry.failureCategory === "bash_cancelled"
    || entry.failureCategory === "grep_cancelled"
  ) {
    return "cancelled";
  }
  if (entry.failureCategory === "approval_denied" || entry.approvalDecision === "deny") {
    return "approval_denied";
  }
  if ((entry.policyReason && unavailableReasons.has(entry.policyReason)) || (entry.failureCategory && unavailableFailures.has(entry.failureCategory))) return "unavailable";
  if (entry.outcomeSummary === "success") return "success";
  if (entry.outcomeSummary === "approval_required" || entry.lifecycleState === "awaiting_approval") {
    return "pending";
  }
  if (entry.outcomeSummary === "in_progress") return "in_progress";
  if (
    entry.outcomeSummary === "denied" &&
    entry.decisionSummary === "deny"
  ) {
    return "policy_denied";
  }
  if (entry.outcomeSummary === "failed") return entry.decisionSummary === "allow" ? "execution_failed" : "failed";
  if (entry.outcomeSummary === "denied") return "denied";
  // This is a defensive presentation branch for a future Runtime outcome;
  // the control-read parser remains the authority for accepted wire values.
  return "in_progress";
}

export function activityOutcomeKey(kind: ActivityOutcomeKind): string {
  if (kind === "failed") return "enum.activityOutcome.failed";
  return `m8Activity.outcome.${kind}`;
}

export function activityOutcomeNeedsAttention(kind: ActivityOutcomeKind): boolean {
  return kind !== "success";
}

/**
 * Give entries an identity for the current bounded snapshot.  Missing IDs and
 * duplicate IDs deliberately remain local to the snapshot, so a refresh
 * cannot silently open a different event in the Inspector.
 */
export function activityRows(entries: ActivityEntry[], snapshotId: number): ActivityRow[] {
  const counts = new Map<string, number>();
  for (const entry of entries) {
    if (entry.eventId) counts.set(entry.eventId, (counts.get(entry.eventId) ?? 0) + 1);
  }
  return entries.map((entry, index) => {
    const stable = Boolean(entry.eventId && counts.get(entry.eventId) === 1);
    return {
      entry,
      rowKey: stable ? `event:${entry.eventId}` : `snapshot:${snapshotId}:${index}`,
      ...(stable ? { stableEventId: entry.eventId } : {}),
      snapshotLocal: !stable,
    };
  });
}

/** Keep an Inspector open only when the same stable Runtime event is present. */
export function reconcileActivitySelection(selection: ActivitySelection | null, rows: ActivityRow[]): ActivitySelection | null {
  if (!selection?.eventId) return null;
  const row = rows.find((candidate) => candidate.stableEventId === selection.eventId);
  return row ? { rowKey: row.rowKey, eventId: row.stableEventId } : null;
}

export function filterActivityRows(rows: ActivityRow[], filter: ActivityFilter): ActivityRow[] {
  if (filter === "all") return rows;
  if (filter === "success") return rows.filter((row) => activityOutcomeKind(row.entry) === "success");
  if (filter === "attention") return rows.filter((row) => activityOutcomeNeedsAttention(activityOutcomeKind(row.entry)));
  const tool = filter.slice("tool:".length);
  return rows.filter((row) => row.entry.toolName === tool);
}

export function activityTools(rows: ActivityRow[]): string[] {
  return [...new Set(rows.map((row) => row.entry.toolName).filter((tool): tool is string => Boolean(tool)))].sort();
}

/** Return only the Runtime-supplied sanitized target/command projection. */
export function activityTarget(entry: ActivityEntry): { kind: "target" | "command"; value: string } | null {
  const argumentsEvidence = entry.sanitizedArguments;
  if (argumentsEvidence) {
    if (argumentsEvidence.tool === "bash") return { kind: "command", value: argumentsEvidence.commandPreview };
    if (argumentsEvidence.tool === "glob") return { kind: "target", value: argumentsEvidence.pattern };
    if (argumentsEvidence.tool === "grep") return { kind: "target", value: argumentsEvidence.logicalScope };
    return { kind: "target", value: argumentsEvidence.logicalPath };
  }
  if (entry.bash?.commandPreview) return { kind: "command", value: entry.bash.commandPreview };
  return null;
}

export function activityDuration(entry: ActivityEntry): number | undefined {
  return entry.durationMs ?? entry.bash?.durationMs;
}

export function activityOutcomeTone(kind: ActivityOutcomeKind): "success" | "warning" | "danger" | "neutral" {
  if (kind === "success") return "success";
  if (kind === "pending" || kind === "in_progress" || kind === "unavailable" || kind === "cancelled" || kind === "expired") return "warning";
  if (kind === "policy_denied" || kind === "approval_denied" || kind === "execution_failed" || kind === "failed" || kind === "denied") return "danger";
  return "neutral";
}
