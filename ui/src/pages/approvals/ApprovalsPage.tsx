import type {
  ApprovalDecision,
  ApprovalRequest,
  ApprovalStatus,
  BashApprovalDetails,
} from "@protocol/v1";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { PageKey, PolicyDraftHandoff } from "@/app/router/routes";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import {
  approvalDecisionKey,
  approvalStatusKey,
  bashClassificationKey,
  bashRiskFlagKey,
  componentStatusKey,
  policyReasonKey,
  presentError,
  riskLevelKey,
  type TranslationKey,
} from "@/i18n/presentation";

type Navigate = (page: PageKey, options?: { policyDraft?: PolicyDraftHandoff }) => void;

function statusTone(status: ApprovalStatus): "success" | "warning" | "danger" | "neutral" {
  if (status === "approved") return "success";
  if (status === "pending") return "warning";
  if (status === "denied" || status === "expired" || status === "cancelled") return "danger";
  return "neutral";
}

type ApprovalErrorState = {
  key: TranslationKey;
  code?: string;
};

function approvalErrorState(error: unknown): ApprovalErrorState {
  const presented = presentError(error);
  return { key: presented.summaryKey, ...(presented.code ? { code: presented.code } : {}) };
}

function approvalItems(data: ReturnType<typeof useHarnessData>["approvals"]): ApprovalRequest[] {
  return data.status === "success" || data.status === "empty" ? data.data.items : [];
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

function sessionUnavailableReasonKey(request: ApprovalRequest): TranslationKey {
  if (request.allowForSessionUnavailableReason === "unclassified_high_risk" || request.bashDetails?.classification === "unclassified_high_risk") {
    return "approvals.sessionUnavailable.unclassifiedHighRisk";
  }
  return "approvals.sessionUnavailable.runtimeScope";
}

function displayStateKey(details: BashApprovalDetails): TranslationKey {
  if (details.previewRedacted && details.previewTruncated) return "approvals.displayState.redactedAndTruncated";
  if (details.previewRedacted) return "approvals.displayState.redacted";
  return "approvals.displayState.truncated";
}

export function ApprovalsPage({ onNavigate }: { onNavigate?: Navigate }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [resolving, setResolving] = useState<string | null>(null);
  const [error, setError] = useState<ApprovalErrorState | null>(null);

  useEffect(() => {
    void data.refreshApprovals();
  }, [data.refreshApprovals]);

  const items = approvalItems(data.approvals);
  const runtimeReady = data.components.runtime === "ready" || data.components.runtime === "degraded";

  const resolve = async (request: ApprovalRequest, decision: ApprovalDecision) => {
    if (request.status !== "pending" || resolving) return;
    setResolving(request.requestId);
    setError(null);
    try {
      await data.client.approvalsResolve(request.requestId, decision);
      await data.refreshApprovals();
    } catch (actionError) {
      setError(approvalErrorState(actionError));
      await data.refreshApprovals().catch(() => undefined);
    } finally {
      setResolving(null);
    }
  };

  const createRule = (request: ApprovalRequest) => {
    const draft: PolicyDraftHandoff = {
      tool: request.tool,
      scope: request.bashDetails?.commandPreview ?? request.scopeSummary,
      name: request.tool === "bash"
        ? t("approvals.draft.bashName")
        : t("approvals.draft.toolName", { tool: request.tool }),
      reason: request.tool === "bash"
        ? t("approvals.draft.bashReason")
        : t("approvals.draft.toolReason", { tool: request.tool }),
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
      eyebrow={t("approvals.eyebrow")}
      title={t("approvals.title")}
      description={t("approvals.description")}
      action={<StatusBadge tone={runtimeReady ? "success" : "danger"}>{t("approvals.runtimeStatus", { status: t(componentStatusKey(data.components.runtime)) })}</StatusBadge>}
    />
    {error && <div className="notice notice-danger" role="alert">
      <strong>{t("approvals.actionFailed")}</strong>
      <span>{t(error.key)}</span>
      {error.code && <span className="mono">{error.code}</span>}
      <span>{t("approvals.refreshHint")}</span>
    </div>}
    {data.approvals.status === "loading" && <Card><p className="muted">{t("approvals.loading")}</p></Card>}
    {(data.approvals.status === "error" || data.approvals.status === "disconnected") && <Card><p className="muted">{t("approvals.unavailable")}</p></Card>}
    {(data.approvals.status === "empty" || ((data.approvals.status === "success") && items.length === 0)) && <Card><p className="muted">{t("approvals.empty")}</p></Card>}
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
  const { t } = useTranslation();
  return <div className="approval-bash-evidence">
    <div className="approval-detail-grid">
      <div><small>{t("approvals.sanitizedCommandPreview")}</small><p className="mono">{details.commandPreview}</p><p className="muted">{t("approvals.classification", { classification: t(bashClassificationKey(details.classification)) })}</p></div>
      <div><small>{t("approvals.workspaceCwd")}</small><p className="mono">{details.cwd}</p><p className="muted">{t("approvals.workspaceCwdHelp")}</p></div>
    </div>
    <div className="approval-detail-grid">
      <div><small>{t("approvals.normalizedSegments")}</small>{details.normalizedSegments.length === 0 ? <p className="muted">{t("approvals.noDisplayableSegments")}</p> : <ul>{details.normalizedSegments.map((segment, index) => <li key={`${segment.executable}-${index}`} className="mono">{segment.executable}{segment.argv.length ? ` ${segment.argv.join(" ")}` : ""}{segment.argvRedacted ? ` [${t("approvals.argvRedacted")}]` : ""}</li>)}</ul>}</div>
      <div><small>{t("approvals.analyzerRiskFlags")}</small>{details.riskFlags.length === 0 ? <p className="muted">{t("approvals.noneReported")}</p> : <ul>{details.riskFlags.map((flag) => <li key={flag} className="mono">{t(bashRiskFlagKey(flag))}</li>)}</ul>}{(details.previewRedacted || details.previewTruncated) && <p className="muted">{t("approvals.displayFacts", { state: t(displayStateKey(details)) })}</p>}</div>
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
  const { t } = useTranslation();
  const pending = request.status === "pending";
  const bash = request.tool === "bash";
  const sessionEligible = isBashSessionEligible(request);
  return <Card className="approval-card">
    <div className="approval-card-header">
      <div><div className="eyebrow">{t("approvals.request", { requestId: request.requestId })}</div><h3 className="approval-title"><span className="mono">{request.tool}</span><StatusBadge tone={statusTone(request.status)}>{t(approvalStatusKey(request.status))}</StatusBadge></h3></div>
      <span className="muted">{t("approvals.risk", { level: t(riskLevelKey(request.riskLevel)) })}</span>
    </div>
    <div className="approval-facts">
      <span><small>{t(bash ? "approvals.sanitizedCommandScope" : "approvals.exactScope")}</small><b className="mono">{request.bashDetails?.commandPreview ?? request.scopeSummary}</b></span>
      <span><small>{t("approvals.runtimeGeneration")}</small><b className="mono">{request.runtimeId} · {request.generationId}</b></span>
      <span><small>{t("approvals.created")}</small><time dateTime={request.createdAt}>{request.createdAt}</time></span>
      <span><small>{t("approvals.expires")}</small><time dateTime={request.expiresAt}>{request.expiresAt}</time></span>
    </div>
    {request.bashDetails && <BashEvidence details={request.bashDetails} />}
    <div className="approval-detail-grid">
      <div><small>{t("approvals.sanitizedArguments")}</small><dl>{Object.entries(request.sanitizedArguments).map(([key, value]) => <div key={key}><dt>{key}</dt><dd className="mono">{value}</dd></div>)}</dl></div>
      <div><small>{t("approvals.policyEvidence")}</small><p>{t("approvals.matchedRules")}: <span className="mono">{request.matchedRuleIds.length ? request.matchedRuleIds.join(", ") : t("approvals.noMatchedRules")}</span></p><p>{t("approvals.reason")}: <span>{t(policyReasonKey(request.policyReason))}</span> <span className="mono">({request.policyReason})</span></p>{bash && <p>{t("approvals.riskLabel")}: <span>{t(riskLevelKey(request.riskLevel))}</span> <span className="mono">({request.riskLevel})</span></p>}</div>
    </div>
    {pending ? <div className="approval-actions" aria-label={t("approvals.actionsLabel", { tool: request.tool })}>
      <button className="button button-primary" disabled={disabled} onClick={() => onResolve("allow_once")}>{resolving ? t("approvals.resolving") : t(approvalDecisionKey("allow_once"))}</button>
      <button className="button button-secondary" disabled={disabled || (bash && !sessionEligible)} onClick={() => onResolve("allow_for_session")}>{t(approvalDecisionKey("allow_for_session"))}</button>
      <button className="button button-quiet danger-text" disabled={disabled} onClick={() => onResolve("deny")}>{t(approvalDecisionKey("deny"))}</button>
      <button className="button button-quiet" disabled={resolving} onClick={onCreateRule}>{t("approvals.createRule")}</button>
      {bash && !sessionEligible && <span className="muted" role="note">{t(sessionUnavailableReasonKey(request))}</span>}
    </div> : <div className="approval-actions"><button className="button button-quiet" disabled>{t("approvals.createRule")}</button><span className="muted">{t("approvals.terminalReadOnly")}</span></div>}
  </Card>;
}
