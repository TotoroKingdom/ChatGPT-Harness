import type {
  ApprovalDecision,
  ApprovalRequest,
  ApprovalStatus,
  BashApprovalDetails,
  BashRiskFlag,
} from "@protocol/v1";
import { useEffect, useState } from "react";
import type { PageKey, PolicyDraftHandoff } from "@/app/router/routes";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";

type Navigate = (page: PageKey, options?: { policyDraft?: PolicyDraftHandoff }) => void;

const statusLabels: Record<ApprovalStatus, string> = {
  pending: "PENDING",
  approved: "APPROVED",
  denied: "DENIED",
  expired: "EXPIRED",
  cancelled: "CANCELLED",
};

function statusTone(status: ApprovalStatus): "success" | "warning" | "danger" | "neutral" {
  if (status === "approved") return "success";
  if (status === "pending") return "warning";
  if (status === "denied" || status === "expired" || status === "cancelled") return "danger";
  return "neutral";
}

function decisionLabel(decision: ApprovalDecision): string {
  switch (decision) {
    case "allow_once": return "Allow once";
    case "allow_for_session": return "Allow for session";
    case "deny": return "Deny";
  }
}

function approvalErrorMessage(error: unknown): string {
  const code = error && typeof error === "object" && "code" in error ? error.code : undefined;
  switch (code) {
    case "APPROVAL_NOT_FOUND": return "This approval no longer exists in the current Runtime generation.";
    case "APPROVAL_ALREADY_RESOLVED": return "This approval is already resolved or expired. Refresh the Approval Center.";
    case "APPROVAL_EXPIRED": return "This approval expired before it could be resolved. Refresh the Approval Center.";
    case "APPROVAL_CANCELLED": return "Runtime cancelled this approval. Refresh the Approval Center.";
    case "RUNTIME_NOT_READY":
    case "RUNTIME_UNAVAILABLE": return "Runtime is not ready. Start Runtime and refresh before resolving approvals.";
    default: return "Runtime did not accept the approval action. Refresh and try again.";
  }
}

function approvalItems(data: ReturnType<typeof useHarnessData>["approvals"]): ApprovalRequest[] {
  return data.status === "success" || data.status === "empty" ? data.data.items : [];
}

function riskFlagLabel(flag: BashRiskFlag): string {
  // Keep the typed flag value visible as evidence; callers can map it to a
  // localized label later without losing the Runtime vocabulary.
  return flag;
}

function safeBashMatcher(request: ApprovalRequest): { executable: string; argvPrefix: string[] } | null {
  const details = request.bashDetails;
  if (!details || details.classification !== "classified" || details.previewRedacted || details.previewTruncated || details.normalizedSegments.length === 0) return null;
  const segment = details.normalizedSegments[0];
  if (segment.argvRedacted || segment.executable.trim() === "" || segment.argv.some((token) => token.includes("\0"))) return null;
  return { executable: segment.executable, argvPrefix: segment.argv };
}

function isBashSessionEligible(request: ApprovalRequest): boolean {
  if (request.tool !== "bash") return true;
  // Bash always carries an explicit eligibility field. Missing data is treated
  // as unavailable so the UI cannot imply a reusable grant from an old DTO.
  // The boolean is Runtime's authoritative grant-eligibility observation.
  // Redacted/truncated display facts only constrain Create Rule prefill; they
  // must not make the UI second-guess an otherwise eligible exact grant.
  return request.allowForSessionEligible === true;
}

function sessionUnavailableReason(request: ApprovalRequest): string {
  if (request.allowForSessionUnavailableReason === "unclassified_high_risk" || request.bashDetails?.classification === "unclassified_high_risk") {
    return "Allow for session is disabled for unclassified high-risk commands. Allow once or create a reviewable rule draft.";
  }
  return "Runtime did not report a safe reusable Bash session scope.";
}

export function ApprovalsPage({ onNavigate }: { onNavigate?: Navigate }) {
  const data = useHarnessData();
  const [resolving, setResolving] = useState<string | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    void data.refreshApprovals();
  }, [data.refreshApprovals]);

  const items = approvalItems(data.approvals);
  const runtimeReady = data.components.runtime === "ready" || data.components.runtime === "degraded";

  const resolve = async (request: ApprovalRequest, decision: ApprovalDecision) => {
    if (request.status !== "pending" || resolving) return;
    setResolving(request.requestId);
    setError("");
    try {
      await data.client.approvalsResolve(request.requestId, decision);
      await data.refreshApprovals();
    } catch (actionError) {
      setError(approvalErrorMessage(actionError));
      await data.refreshApprovals().catch(() => undefined);
    } finally {
      setResolving(null);
    }
  };

  const createRule = (request: ApprovalRequest) => {
    const draft: PolicyDraftHandoff = {
      tool: request.tool,
      scope: request.bashDetails?.commandPreview ?? request.scopeSummary,
      name: request.tool === "bash" ? "Review Bash approvals" : `Review ${request.tool} approvals`,
      reason: request.tool === "bash"
        ? "Review this Bash command scope before allowing future execution. Dynamic or unclassified commands remain at least ASK."
        : `Review requests for ${request.tool} before allowing execution.`,
    };
    const matcher = safeBashMatcher(request);
    if (request.tool === "bash") {
      draft.matcher = matcher
        ? { type: "bash_command", executable: matcher.executable, argvPrefix: matcher.argvPrefix }
        : { type: "tool", tool: "bash" };
    }
    // This only opens a draft. It never resolves the current request or saves
    // a policy rule on behalf of the user.
    onNavigate?.("policies", { policyDraft: draft });
  };

  return <div className="page-stack">
    <SectionHeading
      eyebrow="M5 human approval workflow"
      title="Approval Center"
      description="Review Runtime-generated sanitized facts, then resolve the pending request without recalling the MCP tool."
      action={<StatusBadge tone={runtimeReady ? "success" : "danger"}>{runtimeReady ? "Runtime connected" : "Runtime stopped"}</StatusBadge>}
    />
    {error && <div className="notice notice-danger" role="alert"><strong>Approval action failed</strong><span>{error}</span></div>}
    {data.approvals.status === "loading" && <Card><p className="muted">Loading approvals from Runtime…</p></Card>}
    {(data.approvals.status === "error" || data.approvals.status === "disconnected") && <Card><p className="muted">Approval state is unavailable. Refresh when Runtime is ready.</p></Card>}
    {(data.approvals.status === "empty" || ((data.approvals.status === "success") && items.length === 0)) && <Card><p className="muted">No approval requests in the current Runtime generation.</p></Card>}
    {items.map((request) => <ApprovalCard
      key={request.requestId}
      request={request}
      disabled={!runtimeReady || resolving !== null}
      resolving={resolving === request.requestId}
      onResolve={(decision) => void resolve(request, decision)}
      onCreateRule={() => createRule(request)}
    />)}
  </div>;
}

function BashEvidence({ details }: { details: BashApprovalDetails }) {
  const displayState = details.previewRedacted && details.previewTruncated
    ? "redacted and truncated"
    : details.previewRedacted
      ? "redacted"
      : "truncated";
  return <div className="approval-bash-evidence">
    <div className="approval-detail-grid">
      <div><small>Sanitized command preview</small><p className="mono">{details.commandPreview}</p><p className="muted">Classification: <span className="mono">{details.classification}</span></p></div>
      <div><small>Workspace cwd</small><p className="mono">{details.cwd}</p><p className="muted">Bash uses the active workspace as cwd; this is not a filesystem sandbox.</p></div>
    </div>
    <div className="approval-detail-grid">
      <div><small>Normalized segments</small>{details.normalizedSegments.length === 0 ? <p className="muted">No displayable normalized segments.</p> : <ul>{details.normalizedSegments.map((segment, index) => <li key={`${segment.executable}-${index}`} className="mono">{segment.executable}{segment.argv.length ? ` ${segment.argv.join(" ")}` : ""}{segment.argvRedacted ? " [argv redacted]" : ""}</li>)}</ul>}</div>
      <div><small>Analyzer risk flags</small>{details.riskFlags.length === 0 ? <p className="muted">None reported</p> : <ul>{details.riskFlags.map((flag) => <li key={flag} className="mono">{riskFlagLabel(flag)}</li>)}</ul>}{(details.previewRedacted || details.previewTruncated) && <p className="muted">Display facts are {displayState}; Create Rule cannot prefill a BashCommand matcher from them. Allow for session follows Runtime eligibility.</p>}</div>
    </div>
  </div>;
}

function ApprovalCard({
  request,
  disabled,
  resolving,
  onResolve,
  onCreateRule,
}: {
  request: ApprovalRequest;
  disabled: boolean;
  resolving: boolean;
  onResolve: (decision: ApprovalDecision) => void;
  onCreateRule: () => void;
}) {
  const pending = request.status === "pending";
  const bash = request.tool === "bash";
  const sessionEligible = isBashSessionEligible(request);
  return <Card className="approval-card">
    <div className="approval-card-header">
      <div><div className="eyebrow">Request {request.requestId}</div><h3 className="approval-title"><span className="mono">{request.tool}</span><StatusBadge tone={statusTone(request.status)}>{statusLabels[request.status]}</StatusBadge></h3></div>
      <span className="muted">{request.riskLevel.toUpperCase()} risk</span>
    </div>
    <div className="approval-facts">
      <span><small>{bash ? "Sanitized command scope" : "Exact scope"}</small><b className="mono">{request.bashDetails?.commandPreview ?? request.scopeSummary}</b></span>
      <span><small>Runtime / generation</small><b className="mono">{request.runtimeId} · {request.generationId}</b></span>
      <span><small>Created</small><time dateTime={request.createdAt}>{request.createdAt}</time></span>
      <span><small>Expires</small><time dateTime={request.expiresAt}>{request.expiresAt}</time></span>
    </div>
    {request.bashDetails && <BashEvidence details={request.bashDetails} />}
    <div className="approval-detail-grid">
      <div><small>Sanitized arguments</small><dl>{Object.entries(request.sanitizedArguments).map(([key, value]) => <div key={key}><dt>{key}</dt><dd className="mono">{value}</dd></div>)}</dl></div>
      <div><small>Policy evidence</small><p>Matched rules: <span className="mono">{request.matchedRuleIds.length ? request.matchedRuleIds.join(", ") : "—"}</span></p><p>Reason: <span className="mono">{request.policyReason}</span></p>{bash && <p>Risk: <span className="mono">HIGH</span></p>}</div>
    </div>
    {pending ? <div className="approval-actions" aria-label={`Actions for ${request.tool} approval`}>
      <button className="button button-primary" disabled={disabled} onClick={() => onResolve("allow_once")}>{resolving ? "Resolving…" : decisionLabel("allow_once")}</button>
      <button className="button button-secondary" disabled={disabled || (bash && !sessionEligible)} onClick={() => onResolve("allow_for_session")}>{decisionLabel("allow_for_session")}</button>
      <button className="button button-quiet danger-text" disabled={disabled} onClick={() => onResolve("deny")}>{decisionLabel("deny")}</button>
      <button className="button button-quiet" disabled={resolving} onClick={onCreateRule}>Create rule</button>
      {bash && !sessionEligible && <span className="muted" role="note">{sessionUnavailableReason(request)}</span>}
    </div> : <div className="approval-actions"><button className="button button-quiet" disabled>Create rule</button><span className="muted">Terminal approvals are read-only.</span></div>}
  </Card>;
}
