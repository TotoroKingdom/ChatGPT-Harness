import type {
  ApprovalDecision,
  ApprovalRequest,
  ApprovalStatus,
  BashApprovalDetails,
  RuntimeState,
} from "@protocol/v1";
import type { TFunction } from "i18next";
import { useCallback, useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Navigate, PolicyDraftHandoff } from "@/app/router/routes";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import {
  Button,
  Disclosure,
  InlineFeedback,
  Inspector,
  ObservedTime,
  Row,
  Section,
  SectionHeading,
  StatusBadge,
} from "@/components/common/primitives";
import {
  approvalDecisionKey,
  approvalStatusKey,
  bashDestructiveReasonKey,
  bashClassificationKey,
  bashRiskFlagKey,
  componentStatusKey,
  policyReasonKey,
  presentError,
  riskLevelKey,
  type TranslationKey,
} from "@/i18n/presentation";
import { safeApprovalMatcher } from "./approval-handoff";

type ApprovalNavigate = Navigate;
type ApprovalFeedback = {
  tone: "error" | "success" | "info";
  key?: TranslationKey;
  copyKey?: string;
  detail?: string;
  code?: string;
};
function tx(t: TFunction, key: string, values?: Record<string, string | number>): string {
  return t(`m8Approvals.${key}`, values);
}

function statusTone(status: ApprovalStatus): "success" | "warning" | "danger" | "neutral" {
  if (status === "approved") return "success";
  if (status === "pending") return "warning";
  if (status === "denied" || status === "expired" || status === "cancelled") return "danger";
  return "neutral";
}

function loadedApprovalItems(state: ReturnType<typeof useHarnessData>["approvals"]): ApprovalRequest[] | null {
  return state.status === "success" || state.status === "empty" ? state.data.items : null;
}

function isTerminal(status: ApprovalStatus): boolean {
  return status !== "pending";
}

function runtimeSnapshot(data: ReturnType<typeof useHarnessData>): RuntimeState | null {
  const resource = data.runtime;
  return resource && (resource.status === "success" || resource.status === "empty") ? resource.data : null;
}

function requestIsStale(request: ApprovalRequest, runtime: RuntimeState | null, sessionChanged: boolean): boolean {
  return sessionChanged || (runtime !== null && runtime.runtimeId !== request.runtimeId);
}

type ExpiryInfo = { valid: boolean; expired: boolean; soon: boolean };

function expiryInfo(request: ApprovalRequest, now = Date.now()): ExpiryInfo {
  const timestamp = Date.parse(request.expiresAt);
  if (!Number.isFinite(timestamp)) return { valid: false, expired: false, soon: false };
  const remaining = timestamp - now;
  return { valid: true, expired: remaining <= 0, soon: remaining > 0 && remaining <= 60_000 };
}

function displayState(details: BashApprovalDetails): TranslationKey {
  if (details.previewRedacted && details.previewTruncated) return "displayRedactedTruncated";
  if (details.previewRedacted) return "displayRedacted";
  return "displayTruncated";
}

function riskExplanation(t: TFunction, request: ApprovalRequest): string {
  const details = request.bashDetails;
  const level = request.riskLevel ? t(riskLevelKey(request.riskLevel)) : tx(t, "notReported");
  if (details?.destructive) {
    const reasons = Array.isArray(details.destructiveReasons) ? details.destructiveReasons : [];
    return tx(t, "riskExplanation", {
      level,
      details: reasons.length
        ? reasons.map((reason) => t(bashDestructiveReasonKey(reason))).join(", ")
        : tx(t, "riskDestructive"),
    });
  }
  if (details?.classification === "unclassified_high_risk") {
    return tx(t, "riskExplanation", { level, details: tx(t, "riskUnclassified") });
  }
  const riskFlags = details && Array.isArray(details.riskFlags) ? details.riskFlags : [];
  if (details && riskFlags.length > 0) {
    return tx(t, "riskExplanation", {
      level,
      details: riskFlags.map((flag) => tx(t, "riskFlagImpact", { flag: t(bashRiskFlagKey(flag)) })).join(" "),
    });
  }
  if (details) return tx(t, "riskExplanation", { level, details: tx(t, "riskNotReported") });
  return tx(t, "riskExplanation", {
    level,
    details: request.policyReason ? t(policyReasonKey(request.policyReason)) : tx(t, "notReported"),
  });
}

function previewState(t: TFunction, request: ApprovalRequest): string {
  const details = request.bashDetails;
  if (!details || request.tool !== "bash") return tx(t, "previewUnconfirmed");
  if (
    typeof details.commandPreview !== "string"
    || typeof details.classification !== "string"
    || !Array.isArray(details.normalizedSegments)
    || !Array.isArray(details.riskFlags)
    || (details.destructive !== undefined && typeof details.destructive !== "boolean")
    || (details.destructiveReasons !== undefined && !Array.isArray(details.destructiveReasons))
    || typeof details.cwd !== "string"
    || typeof details.previewRedacted !== "boolean"
    || typeof details.previewTruncated !== "boolean"
  ) return tx(t, "previewUnconfirmed");
  if (!details.previewRedacted && !details.previewTruncated) return tx(t, "previewComplete");
  const state = details.previewRedacted && details.previewTruncated
    ? "displayRedactedTruncated"
    : details.previewRedacted ? "displayRedacted" : "displayTruncated";
  return tx(t, "previewIncomplete", { state: tx(t, state) });
}

function scopeLabel(request: ApprovalRequest): string {
  return request.bashDetails?.commandPreview ?? request.scopeSummary;
}

function approvalSnapshotKey(request: ApprovalRequest): string {
  return JSON.stringify({
    runtimeId: request.runtimeId,
    generationId: request.generationId,
    tool: request.tool,
    scopeSummary: request.scopeSummary,
    expiresAt: request.expiresAt,
    riskLevel: request.riskLevel,
    policyReason: request.policyReason,
    matchedRuleIds: request.matchedRuleIds,
    sanitizedArguments: request.sanitizedArguments,
    allowForSessionEligible: request.allowForSessionEligible,
    allowForSessionUnavailableReason: request.allowForSessionUnavailableReason,
    bashDetails: request.bashDetails,
  });
}

function expectedDecisionStatus(decision: ApprovalDecision): ApprovalStatus {
  return decision === "deny" ? "denied" : "approved";
}

function approvalError(error: unknown): ApprovalFeedback {
  const presented = presentError(error);
  return {
    tone: "error",
    key: presented.summaryKey,
    ...(presented.code ? { code: presented.code } : {}),
  };
}

function feedbackText(t: TFunction, feedback: ApprovalFeedback): string {
  if (feedback.copyKey) return tx(t, feedback.copyKey);
  return feedback.key ? t(feedback.key) : "";
}

function ApprovalActions({
  request,
  disabled,
  resolving,
  sessionEligible,
  decisionEvidenceId,
  onResolve,
  onCreateRule,
}: {
  request: ApprovalRequest;
  disabled: boolean;
  resolving: boolean;
  sessionEligible: boolean;
  decisionEvidenceId?: string;
  onResolve: (decision: ApprovalDecision) => void;
  onCreateRule: () => void;
}) {
  const { t } = useTranslation();
  const stop = (handler: () => void) => (event: React.MouseEvent) => {
    event.stopPropagation();
    handler();
  };
  const locked = disabled || resolving;
  return <div className="approval-actions" aria-label={tx(t, "actionsLabel", { tool: request.tool })}>
    <Button variant="primary" disabled={locked} aria-describedby={decisionEvidenceId} onClick={stop(() => onResolve("allow_once"))}>
      {resolving ? tx(t, "resolving") : tx(t, "allowOnce")}
    </Button>
    <Button variant="secondary" disabled={locked || !sessionEligible} aria-describedby={decisionEvidenceId} onClick={stop(() => onResolve("allow_for_session"))}>
      {tx(t, "allowSession")}
    </Button>
    <Button variant="danger" disabled={locked} aria-describedby={decisionEvidenceId} onClick={stop(() => onResolve("deny"))}>
      {tx(t, "deny")}
    </Button>
    <Button variant="quiet" disabled={resolving} aria-describedby={decisionEvidenceId} onClick={stop(onCreateRule)}>
      {tx(t, "createRule")}
    </Button>
  </div>;
}

function FrontEvidence({ request, expired, soon, runtimeIdentityKnown = true, evidenceId, expiryValid = true, compact = false }: { request: ApprovalRequest; expired: boolean; soon: boolean; runtimeIdentityKnown?: boolean; evidenceId?: string; expiryValid?: boolean; compact?: boolean }) {
  const { t } = useTranslation();
  const details = request.bashDetails;
  const preview = scopeLabel(request);
  const sessionEligible = request.tool !== "bash" || request.allowForSessionEligible === true;
  return <div id={evidenceId} className={`approval-front-evidence ${compact ? "compact" : ""}`}>
    <div className="approval-facts">
      {!compact && <span><small>{tx(t, "target")}</small><b className="mono">{preview}</b></span>}
      <span><small>{tx(t, "why")}</small><b>{request.policyReason ? t(policyReasonKey(request.policyReason)) : tx(t, "notReported")}</b></span>
      <span><small>{tx(t, "risk")}</small><b>{riskExplanation(t, request)}</b></span>
      {!compact && <span><small>{tx(t, "expiryLabel")}</small>{expiryValid && request.expiresAt ? <ObservedTime value={request.expiresAt} fallback={request.expiresAt} /> : <span>{request.expiresAt || tx(t, "notReported")}</span>}</span>}
    </div>
    <p className="muted">{previewState(t, request)}</p>
    {details && <p className="muted"><strong>{tx(t, "classification")}: </strong>{details.classification ? t(bashClassificationKey(details.classification)) : tx(t, "notReported")}</p>}
    {details?.cwd === "active_workspace" && <p className="muted"><strong>{tx(t, "workspace")}: </strong><span className="mono">{details.cwd}</span> · {tx(t, "activeWorkspace")}</p>}
    {details && Array.isArray(details.riskFlags) && details.riskFlags.length > 0 && <p className="muted"><strong>{tx(t, "risk")}: </strong>{details.riskFlags.map((flag) => t(bashRiskFlagKey(flag))).join(", ")}</p>}
    {details?.destructive && <p className="muted"><strong>{tx(t, "destructive")}: </strong>{Array.isArray(details.destructiveReasons) && details.destructiveReasons.length > 0 ? details.destructiveReasons.map((reason) => t(bashDestructiveReasonKey(reason))).join(", ") : tx(t, "riskDestructive")}</p>}
    {details?.destructive && details.classification === "classified" && <p className="muted">{tx(t, "destructiveRuleUnavailable")}</p>}
    {request.tool === "bash" && <p className="muted">{sessionEligible ? tx(t, "sessionScope") : tx(t, "sessionUnavailable", { reason: request.allowForSessionUnavailableReason === "unclassified_high_risk" ? tx(t, "riskUnclassified") : tx(t, "runtimeUnavailable") })}</p>}
    {request.tool !== "bash" && <p className="muted">{tx(t, "fileSessionScope", { tool: request.tool, scope: request.scopeSummary })}</p>}
    {!runtimeIdentityKnown && <InlineFeedback error>{tx(t, "runtimeUnavailable")}</InlineFeedback>}
    {!expiryValid && <InlineFeedback error>{tx(t, "expiryUnconfirmed")}</InlineFeedback>}
    {soon && <InlineFeedback>{tx(t, "expiresSoon")}</InlineFeedback>}
    {expired && <InlineFeedback error>{tx(t, "expiredCheck")}</InlineFeedback>}
  </div>;
}

function AdvancedEvidence({ request }: { request: ApprovalRequest }) {
  const { t } = useTranslation();
  const details = request.bashDetails;
  return <Disclosure title={tx(t, "advanced")}>
    <dl className="approval-detail-grid">
      <div><dt>{tx(t, "requestId")}</dt><dd className="mono">{request.requestId}</dd></div>
      <div><dt>{tx(t, "runtimeId")}</dt><dd className="mono">{request.runtimeId}</dd></div>
      <div><dt>{tx(t, "generationId")}</dt><dd className="mono">{request.generationId}</dd></div>
      <div><dt>{tx(t, "status")}</dt><dd>{t(approvalStatusKey(request.status))}</dd></div>
      <div><dt>{tx(t, "decision")}</dt><dd>{request.decision ? t(approvalDecisionKey(request.decision)) : tx(t, "notReported")}</dd></div>
      <div><dt>{tx(t, "created")}</dt><dd><time dateTime={request.createdAt}>{request.createdAt}</time></dd></div>
      <div><dt>{tx(t, "resolved")}</dt><dd>{request.resolvedAt ? <time dateTime={request.resolvedAt}>{request.resolvedAt}</time> : tx(t, "notReported")}</dd></div>
      <div><dt>{tx(t, "matchedRules")}</dt><dd className="mono">{request.matchedRuleIds.length ? request.matchedRuleIds.join(", ") : tx(t, "notReported")}</dd></div>
      <div><dt>{tx(t, "policyReason")}</dt><dd className="mono">{request.policyReason ?? tx(t, "notReported")}</dd></div>
    </dl>
    <div className="approval-detail-grid">
      <div><small>{tx(t, "sanitizedArguments")}</small><pre className="mono">{JSON.stringify(request.sanitizedArguments, null, 2)}</pre></div>
      {details && <div>
        <small>{tx(t, "preview")}</small>
        <pre className="mono">{details.commandPreview}</pre>
        <p className="muted">{tx(t, "classification")}: {details.classification ? t(bashClassificationKey(details.classification)) : tx(t, "notReported")}</p>
        {details.destructive && <p className="muted">{tx(t, "destructiveReasons")}: {Array.isArray(details.destructiveReasons) && details.destructiveReasons.length > 0 ? details.destructiveReasons.map((reason) => t(bashDestructiveReasonKey(reason))).join(", ") : tx(t, "riskDestructive")}</p>}
        <p className="muted">{tx(t, "workspace")}: {details.cwd}</p>
        {(details.previewRedacted || details.previewTruncated) && <p className="muted">{tx(t, "displayFacts", { state: tx(t, displayState(details)) })}</p>}
      </div>}
    </div>
    {details && <div className="approval-detail-grid">
      <div><small>{tx(t, "normalizedSegments")}</small>{!Array.isArray(details.normalizedSegments) || details.normalizedSegments.length === 0 ? <p className="muted">{tx(t, "noSegments")}</p> : <ul>{details.normalizedSegments.map((segment, index) => <li key={`${segment.executable}-${index}`} className="mono">{segment.executable}{segment.argv.length ? ` ${segment.argv.join(" ")}` : ""}{segment.argvRedacted ? ` [${tx(t, "argvRedacted")}]` : ""}</li>)}</ul>}</div>
      <div><small>{tx(t, "riskFlagsAdvanced")}</small>{!Array.isArray(details.riskFlags) || details.riskFlags.length === 0 ? <p className="muted">{tx(t, "notReported")}</p> : <ul>{details.riskFlags.map((flag) => <li key={flag}>{t(bashRiskFlagKey(flag))}</li>)}</ul>}</div>
    </div>}
  </Disclosure>;
}

function ApprovalInspector({
  request,
  onClose,
  disabled,
  resolving,
  sessionEligible,
  runtimeIdentityKnown,
  stale,
  feedback,
  onResolve,
  onCreateRule,
}: {
  request: ApprovalRequest;
  onClose: () => void;
  disabled: boolean;
  resolving: boolean;
  sessionEligible: boolean;
  runtimeIdentityKnown: boolean;
  stale: boolean;
  feedback?: ApprovalFeedback;
  onResolve: (decision: ApprovalDecision) => void;
  onCreateRule: () => void;
}) {
  const { t } = useTranslation();
  const expiry: ExpiryInfo = request.status === "pending" ? expiryInfo(request) : { valid: true, expired: false, soon: false };
  const evidenceId = useId();
  return <Inspector title={tx(t, "request", { requestId: request.requestId })} onClose={onClose}>
    <div className="page-stack">
      <div><span className="eyebrow">{request.tool}</span><StatusBadge tone={statusTone(request.status)}>{t(approvalStatusKey(request.status))}</StatusBadge></div>
      <h3>{scopeLabel(request)}</h3>
      <FrontEvidence request={request} expired={expiry.expired} soon={expiry.soon} runtimeIdentityKnown={runtimeIdentityKnown} evidenceId={evidenceId} expiryValid={expiry.valid} />
      {request.status === "pending" && <ApprovalActions request={request} disabled={disabled || stale} resolving={resolving} sessionEligible={sessionEligible} decisionEvidenceId={evidenceId} onResolve={onResolve} onCreateRule={onCreateRule} />}
      {feedback && <InlineFeedback error={feedback.tone === "error"}>{feedbackText(t, feedback)}{feedback.detail && <span> {feedback.detail}</span>}{feedback.code && <span className="mono"> {feedback.code}</span>}{feedback.tone === "error" && <span> {tx(t, "refreshHint")}</span>}</InlineFeedback>}
      <AdvancedEvidence request={request} />
      {isTerminal(request.status) && <InlineFeedback>{tx(t, "terminalReadOnly")}</InlineFeedback>}
      <p className="muted">{tx(t, "noExecutionClaim")}</p>
    </div>
  </Inspector>;
}

function PendingRow({
  request,
  selected,
  disabled,
  resolving,
  sessionEligible,
  expired,
  soon,
  expiryValid,
  feedback,
  stale,
  runtimeIdentityKnown,
  onInspect,
  onResolve,
  onCreateRule,
}: {
  request: ApprovalRequest;
  selected: boolean;
  disabled: boolean;
  resolving: boolean;
  sessionEligible: boolean;
  expired: boolean;
  soon: boolean;
  expiryValid: boolean;
  feedback?: ApprovalFeedback;
  stale: boolean;
  runtimeIdentityKnown: boolean;
  onInspect: () => void;
  onResolve: (decision: ApprovalDecision) => void;
  onCreateRule: () => void;
}) {
  const { t } = useTranslation();
  const evidenceId = useId();
  return <article className={`approval-card ${selected ? "selected" : ""}`}>
    <div className="approval-card-head">
      <div>
        <div className="approval-card-title">
          <button type="button" onClick={onInspect}><strong>{request.tool}</strong><StatusBadge tone={statusTone(request.status)} visual={request.status === "pending" ? "warning" : "unknown"}>{t(approvalStatusKey(request.status))}</StatusBadge><span className="details-affordance">{t("common.details")}</span></button>
        </div>
        <div className="approval-target mono">{scopeLabel(request)}</div>
      </div>
      <div className="approval-card-meta">
        <span className={`approval-risk ${request.riskLevel === "high" ? "risk-high" : ""}`}>{tx(t, "risk")}: {request.riskLevel ? t(riskLevelKey(request.riskLevel)) : tx(t, "notReported")}</span>
        <span className="approval-expiry">{tx(t, "expiryLabel")}: <ObservedTime value={request.expiresAt} fallback={request.expiresAt || tx(t, "notReported")} /></span>
      </div>
    </div>
    <div className="approval-card-body">
      <FrontEvidence request={request} expired={expired} soon={soon} runtimeIdentityKnown={runtimeIdentityKnown} evidenceId={evidenceId} expiryValid={expiryValid} compact />
    </div>
    <div className="approval-card-actions">
      <ApprovalActions request={request} disabled={disabled} resolving={resolving} sessionEligible={sessionEligible} decisionEvidenceId={evidenceId} onResolve={onResolve} onCreateRule={onCreateRule} />
      {(feedback || expired || stale) && <div className="approval-row-feedback">
        {feedback && <InlineFeedback error={feedback.tone === "error"}>{feedbackText(t, feedback)}{feedback.detail && <span> {feedback.detail}</span>}{feedback.code && <span className="mono"> {feedback.code}</span>}{feedback.tone === "error" && <span> {tx(t, "refreshHint")}</span>}</InlineFeedback>}
        {expired && <InlineFeedback error>{tx(t, "staleDecision")}</InlineFeedback>}
        {stale && !feedback && <InlineFeedback error>{tx(t, "requestStale")}</InlineFeedback>}
      </div>}
    </div>
  </article>;
}

function HistoryRow({ request, selected, feedback, onInspect }: { request: ApprovalRequest; selected: boolean; feedback?: ApprovalFeedback; onInspect: () => void }) {
  const { t } = useTranslation();
  return <>
    <Row
    selected={selected}
    onInspect={onInspect}
    title={<><strong className="mono">{request.tool}</strong><StatusBadge tone={statusTone(request.status)}>{t(approvalStatusKey(request.status))}</StatusBadge></>}
    summary={<><span className="mono">{scopeLabel(request)}</span><ObservedTime value={request.resolvedAt ?? request.expiresAt} fallback={request.resolvedAt ?? request.expiresAt} /></>}
    controls={<span className="muted">{tx(t, "terminalReadOnly")}</span>}
    />
    {feedback && <InlineFeedback error={feedback.tone === "error"}>{feedbackText(t, feedback)}{feedback.detail && <span> {feedback.detail}</span>}{feedback.code && <span className="mono"> {feedback.code}</span>}</InlineFeedback>}
  </>;
}

export function ApprovalsPage({ onNavigate, initialRequestId }: { onNavigate?: ApprovalNavigate; initialRequestId?: string }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const incomingItems = loadedApprovalItems(data.approvals);
  const [snapshot, setSnapshot] = useState<ApprovalRequest[]>(incomingItems ?? []);
  const [overrides, setOverrides] = useState<Record<string, ApprovalRequest>>({});
  const [selectedId, setSelectedId] = useState<string | null>(initialRequestId ?? null);
  const [resolvingIds, setResolvingIds] = useState<Set<string>>(() => new Set());
  const [feedback, setFeedback] = useState<Record<string, ApprovalFeedback | undefined>>({});
  const [expiryHints, setExpiryHints] = useState<Set<string>>(() => new Set());
  const [, setExpiryTick] = useState(0);
  const [refreshing, setRefreshing] = useState(false);
  const [sessionChanged, setSessionChanged] = useState(false);
  const sessionRef = useRef<string | null | undefined>(undefined);
  const expiryRefreshRef = useRef<Set<string>>(new Set());
  const runtimeRefreshRef = useRef<string | null>(null);
  const resolvingRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    if (!incomingItems) return;
    setSnapshot(incomingItems);
    setSessionChanged((current) => current ? false : current);
    setOverrides((current) => {
      const next = { ...current };
      for (const id of Object.keys(next)) {
        const observed = incomingItems.find((item) => item.requestId === id);
        if (!observed || observed.status !== "pending") delete next[id];
      }
      return next;
    });
    setExpiryHints((current) => {
      const next = new Set(current);
      for (const id of current) {
        const observed = incomingItems.find((item) => item.requestId === id);
        if (!observed || observed.status !== "pending" || !expiryInfo(observed).expired) {
          next.delete(id);
          expiryRefreshRef.current.delete(id);
        }
      }
      return next;
    });
  }, [data.approvals, incomingItems]);

  useEffect(() => {
    if (initialRequestId) setSelectedId(initialRequestId);
  }, [initialRequestId]);

  const currentRuntime = runtimeSnapshot(data);
  const activeSessionId = data.harness?.activeSessionId ?? null;
  useEffect(() => {
    if (sessionRef.current === undefined) {
      sessionRef.current = activeSessionId;
      return;
    }
    if (sessionRef.current !== activeSessionId) {
      sessionRef.current = activeSessionId;
      setSessionChanged(true);
      void data.refreshApprovals().catch(() => undefined);
    }
  }, [activeSessionId, data.refreshApprovals]);

  useEffect(() => {
    if (!currentRuntime?.runtimeId) return;
    if (runtimeRefreshRef.current === null) {
      runtimeRefreshRef.current = currentRuntime.runtimeId;
      return;
    }
    if (runtimeRefreshRef.current !== currentRuntime.runtimeId) {
      runtimeRefreshRef.current = currentRuntime.runtimeId;
      setSessionChanged(true);
      void data.refreshApprovals().catch(() => undefined);
    }
  }, [currentRuntime?.runtimeId, data.refreshApprovals]);

  const baseItems = incomingItems ?? snapshot;
  const items = useMemo(() => baseItems.map((item) => overrides[item.requestId] ?? item), [baseItems, overrides]);
  const pendingItems = items.filter((item) => item.status === "pending");
  const historyItems = items.filter((item) => isTerminal(item.status));
  const hasRetainedContent = items.length > 0;
  const disconnected = data.approvals.status === "error" || data.approvals.status === "disconnected" || data.approvals.status === "idle";
  const runtimeMismatch = currentRuntime !== null && pendingItems.some((item) => item.runtimeId !== currentRuntime.runtimeId);
  const stalePage = sessionChanged || runtimeMismatch || (disconnected && hasRetainedContent);
  const runtimeReady = data.components.runtime === "ready" || data.components.runtime === "degraded";
  const runtimeIdentityKnown = currentRuntime !== null && Boolean(currentRuntime.runtimeId);
  const authoritativeEmpty = data.approvals.status === "empty" || (data.approvals.status === "success" && incomingItems?.length === 0);

  useEffect(() => {
    void data.refreshApprovals().catch(() => undefined);
  }, [data.refreshApprovals]);

  useEffect(() => {
    const timers: number[] = [];
    for (const request of pendingItems) {
      const expiry = expiryInfo(request);
      if (expiry.expired) {
        setExpiryHints((current) => {
          if (current.has(request.requestId)) return current;
          const next = new Set(current); next.add(request.requestId); return next;
        });
        if (!expiryRefreshRef.current.has(request.requestId)) {
          expiryRefreshRef.current.add(request.requestId);
          void data.refreshApprovals().catch(() => undefined);
        }
      } else {
        const expiryAt = Date.parse(request.expiresAt);
        const soonDelay = expiryAt - 60_000 - Date.now();
        if (Number.isFinite(soonDelay) && soonDelay > 0) {
          timers.push(window.setTimeout(() => setExpiryTick((value) => value + 1), soonDelay));
        }
        const delay = expiryAt - Date.now();
        if (Number.isFinite(delay) && delay > 0) {
          timers.push(window.setTimeout(() => {
            setExpiryHints((current) => {
              const next = new Set(current); next.add(request.requestId); return next;
            });
            if (!expiryRefreshRef.current.has(request.requestId)) {
              expiryRefreshRef.current.add(request.requestId);
              void data.refreshApprovals().catch(() => undefined);
            }
          }, delay));
        }
      }
    }
    return () => timers.forEach((timer) => window.clearTimeout(timer));
  }, [data.client, data.refreshApprovals, pendingItems]);

  const manualRefresh = useCallback(async () => {
    if (refreshing) return;
    setRefreshing(true);
    try { await data.refreshApprovals(); }
    finally { setRefreshing(false); }
  }, [data.refreshApprovals, refreshing]);

  const resolve = useCallback(async (request: ApprovalRequest, decision: ApprovalDecision) => {
    if (request.status !== "pending" || resolvingRef.current.has(request.requestId)) return;
    if (!runtimeIdentityKnown || requestIsStale(request, currentRuntime, sessionChanged) || !runtimeReady || expiryHints.has(request.requestId)) return;
    resolvingRef.current.add(request.requestId);
    setResolvingIds((current) => new Set(current).add(request.requestId));
    setFeedback((current) => ({ ...current, [request.requestId]: undefined }));
    try {
      const initialHarness = data.client.getHarnessStatus();
      const initialSessionId = initialHarness.activeSessionId ?? null;
      const [observedRuntime] = await Promise.all([
        data.client.runtimeStatus(),
        data.client.approvalsList(),
      ]);
      // Read again after both async observations. A session/runtime event can
      // arrive while either first response is in flight; stale replies must
      // never authorize a decision for the old identity.
      const [latestRuntime, latestApprovals] = await Promise.all([
        data.client.runtimeStatus(),
        data.client.approvalsList(),
      ]);
      const currentHarness = data.client.getHarnessStatus();
      const currentComponents = data.client.getComponentStates();
      const currentSessionId = currentHarness.activeSessionId ?? null;
      const sessionChangedDuringPreflight = initialSessionId !== currentSessionId;
      const runtimeChangedDuringPreflight = observedRuntime.runtimeId !== latestRuntime.runtimeId;
      const runtimeUnavailableDuringPreflight = currentComponents.runtime !== "ready" && currentComponents.runtime !== "degraded";
      const latest = latestApprovals.items.find((item) => item.requestId === request.requestId);
      if (!latest || latest.status !== "pending") {
        setFeedback((current) => ({ ...current, [request.requestId]: { tone: "error", key: "errors.codes.approvalAlreadyResolved" } }));
        await data.refreshApprovals().catch(() => undefined);
        return;
      }
      if (sessionChangedDuringPreflight || runtimeChangedDuringPreflight) {
        setFeedback((current) => ({ ...current, [request.requestId]: { tone: "error", copyKey: "requestStale" } }));
        setSessionChanged(true);
        await data.refreshApprovals().catch(() => undefined);
        return;
      }
      if (!latestRuntime.runtimeId || runtimeUnavailableDuringPreflight) {
        setFeedback((current) => ({ ...current, [request.requestId]: { tone: "error", copyKey: "runtimeUnavailable" } }));
        await data.refreshApprovals().catch(() => undefined);
        return;
      }
      const latestExpiry = expiryInfo(latest);
      if (!latestExpiry.valid || latestExpiry.expired) {
        if (latestExpiry.expired) setExpiryHints((current) => new Set(current).add(request.requestId));
        setFeedback((current) => ({ ...current, [request.requestId]: { tone: "error", copyKey: latestExpiry.valid ? "expiredCheck" : "expiryUnconfirmed" } }));
        await data.refreshApprovals().catch(() => undefined);
        return;
      }
      if (
        latest.runtimeId !== latestRuntime.runtimeId
        || latest.runtimeId !== request.runtimeId
        || approvalSnapshotKey(latest) !== approvalSnapshotKey(request)
      ) {
        setFeedback((current) => ({ ...current, [request.requestId]: { tone: "error", copyKey: "requestStale" } }));
        setSessionChanged(true);
        await data.refreshApprovals().catch(() => undefined);
        return;
      }
      const result = await data.client.approvalsResolve(request.requestId, decision);
      if (
        result.approval.requestId !== request.requestId
        || result.approval.runtimeId !== latestRuntime.runtimeId
        || result.approval.status !== expectedDecisionStatus(decision)
        || result.approval.decision !== decision
      ) {
        setFeedback((current) => ({ ...current, [request.requestId]: { tone: "error", copyKey: "decisionFailed" } }));
        await data.refreshApprovals().catch(() => undefined);
        return;
      }
      setOverrides((current) => ({ ...current, [request.requestId]: result.approval }));
      setFeedback((current) => ({ ...current, [request.requestId]: { tone: "success", copyKey: "decisionSubmitted" } }));
      await data.refreshApprovals().catch(() => undefined);
    } catch (error) {
      const next = approvalError(error);
      setFeedback((current) => ({ ...current, [request.requestId]: { ...next, detail: tx(t, "decisionFailed") } }));
      await data.refreshApprovals().catch(() => undefined);
    } finally {
      resolvingRef.current.delete(request.requestId);
      setResolvingIds((current) => {
        const next = new Set(current); next.delete(request.requestId); return next;
      });
    }
  }, [currentRuntime, data, expiryHints, runtimeIdentityKnown, runtimeReady, sessionChanged, t]);

  const createRule = useCallback((request: ApprovalRequest) => {
    const matcher = safeApprovalMatcher(request);
    const broadScope = request.tool !== "bash";
    const draft: PolicyDraftHandoff = {
      tool: request.tool,
      scope: scopeLabel(request),
      name: request.tool === "bash" ? t("approvals.draft.bashName") : t("approvals.draft.toolName", { tool: request.tool }),
      reason: broadScope
        ? `${t("approvals.draft.toolReason", { tool: request.tool })} ${tx(t, "wholeToolScope", { tool: request.tool })}`
        : t("approvals.draft.bashReason"),
      ...(matcher ? { matcher } : {}),
      approvalRequestId: request.requestId,
    };
    const feedbackKey = matcher || broadScope
      ? "ruleDraftPrepared"
      : request.bashDetails?.destructive === true
        ? "destructiveRuleUnavailable"
        : "unsafeRuleDraft";
    setFeedback((current) => ({ ...current, [request.requestId]: { tone: "info", copyKey: feedbackKey } }));
    onNavigate?.("policies", { policyDraft: draft });
  }, [onNavigate, t]);

  const selected = selectedId ? items.find((item) => item.requestId === selectedId) : undefined;
  const requestDisabled = (request: ApprovalRequest) => stalePage || !runtimeReady || !runtimeIdentityKnown || !expiryInfo(request).valid || requestIsStale(request, currentRuntime, sessionChanged) || expiryHints.has(request.requestId);
  const hasKnownQueue = hasRetainedContent || authoritativeEmpty;
  const historyTitle = hasKnownQueue ? `${tx(t, "history")} · ${tx(t, "historyCount", { count: historyItems.length })}` : tx(t, "history");

  return <div className="page-stack">
    <h2 className="sr-only">{t("approvals.title")}</h2>
    <div className="approval-summary-strip">
      <div className="approval-summary-main">
        <div><strong className="approval-count-large">{hasKnownQueue ? pendingItems.length : "—"}</strong><span className="muted">{tx(t, "pendingCount", { count: pendingItems.length })}</span></div>
        <p>{tx(t, "pendingDescription")}</p>
      </div>
      <div className="row-wrap">
        <StatusBadge tone={runtimeReady ? "success" : "danger"} visual={runtimeReady ? "ready" : "error"}>{t("approvals.runtimeStatus", { status: t(componentStatusKey(data.components.runtime)) })}</StatusBadge>
        <Button variant="quiet" onClick={() => void manualRefresh()} disabled={refreshing}>{refreshing ? tx(t, "refreshing") : tx(t, "refresh")}</Button>
      </div>
    </div>
    {stalePage && <div className="notice notice-warning" role="status"><strong>{tx(t, sessionChanged ? "sessionChanged" : runtimeMismatch ? "runtimeChanged" : "stale")}</strong><span>{tx(t, "staleDecision")}</span></div>}
    {(data.approvals.status === "error" || data.approvals.status === "disconnected") && !hasRetainedContent && <div className="notice notice-danger" role="alert">{tx(t, "unavailable")}</div>}
    {data.approvals.status === "loading" && !hasRetainedContent && <InlineFeedback>{tx(t, "loading")}</InlineFeedback>}
    <Section title={tx(t, "pending")}>
      {pendingItems.length === 0 && authoritativeEmpty && <InlineFeedback>{tx(t, "noPending")}</InlineFeedback>}
      {pendingItems.length === 0 && !authoritativeEmpty && !hasRetainedContent && <InlineFeedback>{tx(t, "loading")}</InlineFeedback>}
      {pendingItems.length > 0 && <div className="approval-list">{pendingItems.map((request) => {
        const expiry = expiryInfo(request);
        const expired = expiryHints.has(request.requestId) || expiry.expired;
        return <PendingRow key={request.requestId} request={request} selected={selectedId === request.requestId} disabled={requestDisabled(request)} resolving={resolvingIds.has(request.requestId)} sessionEligible={request.tool !== "bash" || request.allowForSessionEligible === true} expired={expired} soon={expiry.soon} expiryValid={expiry.valid} feedback={feedback[request.requestId]} stale={requestIsStale(request, currentRuntime, sessionChanged)} runtimeIdentityKnown={runtimeIdentityKnown} onInspect={() => setSelectedId(request.requestId)} onResolve={(decision) => void resolve(request, decision)} onCreateRule={() => createRule(request)} />;
      })}</div>}
    </Section>
    <Disclosure title={historyTitle}>
      {historyItems.length === 0 ? <InlineFeedback>{tx(t, "historyEmpty")}</InlineFeedback> : historyItems.map((request) => <HistoryRow key={request.requestId} request={request} feedback={feedback[request.requestId]} selected={selectedId === request.requestId} onInspect={() => setSelectedId(request.requestId)} />)}
    </Disclosure>
    {selected && <ApprovalInspector
      request={selected}
      onClose={() => setSelectedId(null)}
      disabled={requestDisabled(selected)}
      resolving={resolvingIds.has(selected.requestId)}
      sessionEligible={selected.tool !== "bash" || selected.allowForSessionEligible === true}
      runtimeIdentityKnown={runtimeIdentityKnown}
      stale={requestIsStale(selected, currentRuntime, sessionChanged)}
      feedback={feedback[selected.requestId]}
      onResolve={(decision) => void resolve(selected, decision)}
      onCreateRule={() => createRule(selected)}
    />}
  </div>;
}
