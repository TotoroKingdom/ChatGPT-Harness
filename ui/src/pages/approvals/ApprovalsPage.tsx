import type { ApprovalDecision, ApprovalRequest, ApprovalStatus } from "@protocol/v1";
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
    onNavigate?.("policies", {
      policyDraft: {
        tool: request.tool,
        scope: request.scopeSummary,
        name: `Review ${request.tool} approvals`,
        reason: `Review requests for ${request.tool} before allowing execution.`,
      },
    });
  };

  return <div className="page-stack">
    <SectionHeading
      eyebrow="M4 human approval workflow"
      title="Approval Center"
      description="Review the exact Runtime-generated scope, then resolve the pending request without recalling the MCP tool."
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
  return <Card className="approval-card">
    <div className="approval-card-header">
      <div><div className="eyebrow">Request {request.requestId}</div><h3 className="approval-title"><span className="mono">{request.tool}</span><StatusBadge tone={statusTone(request.status)}>{statusLabels[request.status]}</StatusBadge></h3></div>
      <span className="muted">{request.riskLevel.toUpperCase()} risk</span>
    </div>
    <div className="approval-facts">
      <span><small>Exact scope</small><b className="mono">{request.scopeSummary}</b></span>
      <span><small>Runtime / generation</small><b className="mono">{request.runtimeId} · {request.generationId}</b></span>
      <span><small>Created</small><time dateTime={request.createdAt}>{request.createdAt}</time></span>
      <span><small>Expires</small><time dateTime={request.expiresAt}>{request.expiresAt}</time></span>
    </div>
    <div className="approval-detail-grid">
      <div><small>Sanitized arguments</small><dl>{Object.entries(request.sanitizedArguments).map(([key, value]) => <div key={key}><dt>{key}</dt><dd className="mono">{value}</dd></div>)}</dl></div>
      <div><small>Policy evidence</small><p>Matched rules: <span className="mono">{request.matchedRuleIds.length ? request.matchedRuleIds.join(", ") : "—"}</span></p><p>Reason: <span className="mono">{request.policyReason}</span></p></div>
    </div>
    {pending ? <div className="approval-actions" aria-label={`Actions for ${request.tool} approval`}>
      <button className="button button-primary" disabled={disabled} onClick={() => onResolve("allow_once")}>{resolving ? "Resolving…" : decisionLabel("allow_once")}</button>
      <button className="button button-secondary" disabled={disabled} onClick={() => onResolve("allow_for_session")}>{decisionLabel("allow_for_session")}</button>
      <button className="button button-quiet danger-text" disabled={disabled} onClick={() => onResolve("deny")}>{decisionLabel("deny")}</button>
      <button className="button button-quiet" disabled={resolving} onClick={onCreateRule}>Create rule</button>
    </div> : <div className="approval-actions"><button className="button button-quiet" disabled>Create rule</button><span className="muted">Terminal approvals are read-only.</span></div>}
  </Card>;
}
