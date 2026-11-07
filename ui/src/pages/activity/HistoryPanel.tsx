import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type {
  EvidenceCoverage,
  EvidenceStoreStatus,
  HistoryApprovalState,
  HistoryGetResult,
  HistoryOutcome,
  HistoryQueryParams,
  HistoryRequestSummary,
  PolicyDecision,
  TraceGetResult,
} from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import {
  Button,
  Card,
  Disclosure,
  InlineFeedback,
  Inspector,
  ObservedTime,
  Row,
  SectionHeading,
  Select,
  StatusBadge,
} from "@/components/common/primitives";

type TimeRange = "24h" | "7d" | "30d" | "all";

type Filters = {
  range: TimeRange;
  tool: string;
  outcome: HistoryOutcome | "";
  policyDecision: PolicyDecision | "";
  approvalState: HistoryApprovalState | "";
};

const initialFilters: Filters = {
  range: "24h",
  tool: "",
  outcome: "",
  policyDecision: "",
  approvalState: "",
};

const tools = ["read_file", "glob", "grep", "write", "edit", "bash"];
const outcomes: HistoryOutcome[] = [
  "in_progress",
  "success",
  "failed",
  "denied",
  "approval_required",
  "cancelled",
  "timed_out",
  "approval_expired",
  "approval_cancelled",
];
const approvalStates: HistoryApprovalState[] = [
  "none",
  "required_observed",
  "allow_once",
  "allow_for_session",
  "denied",
  "expired",
  "cancelled",
  "incomplete",
  "unknown",
];

function rangeStart(range: TimeRange): string | undefined {
  if (range === "all") return undefined;
  const hours = range === "24h" ? 24 : range === "7d" ? 24 * 7 : 24 * 30;
  return new Date(Date.now() - hours * 60 * 60 * 1000).toISOString();
}

function queryFor(filters: Filters): HistoryQueryParams {
  return {
    ...(rangeStart(filters.range) ? { fromUtc: rangeStart(filters.range) } : {}),
    ...(filters.tool ? { tool: filters.tool } : {}),
    ...(filters.outcome ? { outcome: filters.outcome } : {}),
    ...(filters.policyDecision ? { policyDecision: filters.policyDecision } : {}),
    ...(filters.approvalState ? { approvalState: filters.approvalState } : {}),
    limit: 50,
  };
}

function coverageTone(value: EvidenceCoverage): "success" | "warning" | "danger" {
  if (value === "COMPLETE") return "success";
  if (value === "INCOMPLETE") return "warning";
  return "danger";
}

export function HistoryPanel() {
  const { t } = useTranslation();
  const { client } = useHarnessData();
  const [filters, setFilters] = useState<Filters>(initialFilters);
  const [items, setItems] = useState<HistoryRequestSummary[]>([]);
  const [status, setStatus] = useState<EvidenceStoreStatus | null>(null);
  const [nextCursor, setNextCursor] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<HistoryGetResult | null>(null);
  const [trace, setTrace] = useState<TraceGetResult | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const requestGeneration = useRef(0);

  const query = useMemo(() => queryFor(filters), [filters]);

  const load = useCallback(async (append: boolean) => {
    const generation = ++requestGeneration.current;
    setLoading(true);
    setError(null);
    try {
      const result = await client.historyQuery({
        ...query,
        ...(append && nextCursor ? { cursor: nextCursor } : {}),
      });
      if (generation !== requestGeneration.current) return;
      setStatus(result.storeStatus);
      setItems((current) => append ? [...current, ...result.items] : result.items);
      setNextCursor(result.nextCursor);
    } catch (failure) {
      if (generation === requestGeneration.current) setError(typeof failure === "object" && failure !== null && "code" in failure
        ? String(failure.code)
        : "EVIDENCE_IO");
    } finally {
      if (generation === requestGeneration.current) setLoading(false);
    }
  }, [client, nextCursor, query]);

  useEffect(() => {
    setNextCursor(undefined);
    void load(false);
    // A filter change starts a new keyset sequence.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, client]);

  const inspect = async (item: HistoryRequestSummary) => {
    setDetailLoading(true);
    setError(null);
    try {
      setTrace(null);
      setSelected(await client.historyGet({ runtimeId: item.runtimeId, requestId: item.requestId }));
    } catch (failure) {
      setError(typeof failure === "object" && failure !== null && "code" in failure
        ? String(failure.code)
        : "EVIDENCE_IO");
    } finally {
      setDetailLoading(false);
    }
  };

  const warning = status && (status.state !== "healthy"
    ? t(`m03History.storeState.${status.state}`)
    : status.dropCount > 0
      ? t("m03History.dropWarning", { count: status.dropCount })
      : null);

  return <>
    <Card className="activity-card">
      <div className="filter-toolbar" role="toolbar" aria-label={t("m03History.list")}>
        <Select label={t("m03History.timeRange")} value={filters.range} onChange={(event) => setFilters({ ...filters, range: event.target.value as TimeRange })}>
          <option value="24h">{t("m03History.range.24h")}</option>
          <option value="7d">{t("m03History.range.7d")}</option>
          <option value="30d">{t("m03History.range.30d")}</option>
          <option value="all">{t("m03History.range.all")}</option>
        </Select>
        <Select label={t("m03History.tool")} value={filters.tool} onChange={(event) => setFilters({ ...filters, tool: event.target.value })}>
          <option value="">{t("m03History.all")}</option>
          {tools.map((tool) => <option key={tool} value={tool}>{tool}</option>)}
        </Select>
        <Select label={t("m03History.outcomeLabel")} value={filters.outcome} onChange={(event) => setFilters({ ...filters, outcome: event.target.value as Filters["outcome"] })}>
          <option value="">{t("m03History.all")}</option>
          {outcomes.map((outcome) => <option key={outcome} value={outcome}>{t(`m03History.outcome.${outcome}`)}</option>)}
        </Select>
        <Select label={t("m03History.policy")} value={filters.policyDecision} onChange={(event) => setFilters({ ...filters, policyDecision: event.target.value as Filters["policyDecision"] })}>
          <option value="">{t("m03History.all")}</option>
          <option value="allow">{t("enum.policyDecision.allow")}</option>
          <option value="ask">{t("enum.policyDecision.ask")}</option>
          <option value="deny">{t("enum.policyDecision.deny")}</option>
        </Select>
        <Select label={t("m03History.approval")} value={filters.approvalState} onChange={(event) => setFilters({ ...filters, approvalState: event.target.value as Filters["approvalState"] })}>
          <option value="">{t("m03History.all")}</option>
          {approvalStates.map((state) => <option key={state} value={state}>{t(`m03History.approvalState.${state}`)}</option>)}
        </Select>
        <Button variant="quiet" onClick={() => setFilters(initialFilters)}>{t("m03History.reset")}</Button>
      </div>

      {status && <div className={`notice ${status.state === "healthy" ? "notice-success" : "notice-warning"}`} role="status">
        <strong>{t("m03History.store")}</strong>
        <span>{warning ?? t("m03History.storeState.healthy")}</span>
        {status.footprintBytes !== undefined && <span>{t("m03History.footprint", { bytes: status.footprintBytes })}</span>}
      </div>}
      {error && <div className="notice notice-warning" role="alert"><strong>{t("m03History.error")}</strong><code>{error}</code></div>}
      {loading && items.length === 0 && <InlineFeedback>{t("m03History.loading")}</InlineFeedback>}
      {!loading && !error && items.length === 0 && <div className="empty-inline"><strong>{t("m03History.empty")}</strong><p>{t("m03History.emptyHelp")}</p></div>}
      {items.length > 0 && <div className="activity-grid" aria-label={t("m03History.list")}>
        <div className="activity-grid-head history-grid" aria-hidden="true"><span>{t("m03History.firstSeen")}</span><span>{t("m03History.tool")}</span><span>{t("m03History.outcomeLabel")}</span><span>{t("m03History.policy")} / {t("m03History.approval")}</span><span>{t("m03History.coverage")}</span></div>
        {items.map((item) => <article className="activity-row" data-testid={`history-row-${item.runtimeId}-${item.requestId}`} key={`${item.runtimeId}:${item.requestId}`}>
          <button type="button" className="activity-row-button history-grid" onClick={() => void inspect(item)}>
            <ObservedTime className="cell-time" value={item.firstSeenAtUtc} fallback={item.firstSeenAtUtc} />
            <strong className="cell-tool">{item.toolName}</strong>
            <span className="cell-outcome">{item.outcome ? t(`m03History.outcome.${item.outcome}`) : t(`m03History.terminal.${item.terminalState}`)}</span>
            <span className="cell-target">{item.policyDecision?.toUpperCase() ?? t("m03History.notRecorded")} · {t(`m03History.approvalState.${item.approvalState}`)}</span>
            <span className="cell-coverage"><StatusBadge tone={coverageTone(item.evidenceCoverage)} visual={item.evidenceCoverage === "COMPLETE" ? "success" : item.evidenceCoverage === "INCOMPLETE" ? "warning" : "danger"}>{item.evidenceCoverage}</StatusBadge></span>
          </button>
        </article>)}
      </div>}
      {nextCursor && <div className="settings-actions"><Button variant="secondary" onClick={() => void load(true)} disabled={loading}>{loading ? t("m03History.loading") : t("m03History.loadMore")}</Button></div>}
    </Card>
    {detailLoading && <InlineFeedback>{t("m03History.loadingDetail")}</InlineFeedback>}
    {selected && <HistoryInspector result={selected} onClose={() => { setSelected(null); setTrace(null); }} onTrace={async () => {
      try {
        setTrace(await client.traceGet({ runtimeId: selected.summary.runtimeId, requestId: selected.summary.requestId }));
      } catch (failure) {
        setError(typeof failure === "object" && failure !== null && "code" in failure ? String(failure.code) : "EVIDENCE_IO");
      }
    }} />}
    {trace && <TraceInspector result={trace} onClose={() => setTrace(null)} />}
  </>;
}

function HistoryInspector({ result, onClose, onTrace }: { result: HistoryGetResult; onClose: () => void; onTrace: () => void | Promise<void> }) {
  const { t } = useTranslation();
  const item = result.summary;
  return <Inspector title={t("m03History.detailTitle")} onClose={onClose}>
    <section className="workspace-section">
      <SectionHeading title={t("m03History.request")} />
      <Button variant="secondary" onClick={() => void onTrace()}>{t("m08Trace.open")}</Button>
      <dl className="detail-list">
        <Fact label={t("m03History.tool")}>{item.toolName}</Fact>
        <Fact label={t("m03History.requestId")}><code>{item.requestId}</code></Fact>
        <Fact label={t("m03History.runtimeId")}><code>{item.runtimeId}</code></Fact>
        <Fact label={t("m03History.firstSeen")}><ObservedTime value={item.firstSeenAtUtc} fallback={item.firstSeenAtUtc} /></Fact>
        <Fact label={t("m03History.lastSeen")}><ObservedTime value={item.lastSeenAtUtc} fallback={item.lastSeenAtUtc} /></Fact>
        <Fact label={t("m03History.outcomeLabel")}>{item.outcome ? t(`m03History.outcome.${item.outcome}`) : t(`m03History.terminal.${item.terminalState}`)}</Fact>
        <Fact label={t("m03History.policy")}>{item.policyDecision?.toUpperCase() ?? t("m03History.notRecorded")}</Fact>
        <Fact label={t("m03History.duration")}>{item.durationMs === undefined ? t("m03History.notRecorded") : `${item.durationMs} ms`}</Fact>
        <Fact label={t("m03History.coverage")}><StatusBadge tone={coverageTone(item.evidenceCoverage)}>{item.evidenceCoverage}</StatusBadge></Fact>
      </dl>
    </section>
    {result.terminalApproval && <section className="workspace-section">
      <SectionHeading title={t("m03History.terminalApproval")} />
      <p>{t(`m03History.approvalState.${result.terminalApproval.terminalState}`)}</p>
      {result.terminalApproval.decision && <p>{t("m03History.decision")}: <code>{result.terminalApproval.decision}</code></p>}
    </section>}
    <section className="workspace-section">
      <SectionHeading title={t("m03History.timeline")} />
      <ol className="history-timeline">
        {result.events.map((event) => <li key={event.eventId}>
          <strong>{t(`m03History.stage.${event.stage}`)}</strong>
          <ObservedTime value={event.occurredAtUtc} fallback={event.occurredAtUtc} />
          <span>{t(`m03History.outcome.${event.outcome}`)}</span>
        </li>)}
      </ol>
    </section>
    {result.notice && <div className="notice notice-warning" role="status">
      {t("m03History.notice", { coverage: result.notice.coverage, code: result.notice.code })}
    </div>}
    <Disclosure title={t("m03History.advanced")}>
      <pre>{JSON.stringify(result, null, 2)}</pre>
    </Disclosure>
  </Inspector>;
}

function TraceInspector({ result, onClose }: { result: TraceGetResult; onClose: () => void }) {
  const { t } = useTranslation();
  const tone = result.coverage === "complete" ? "success" : result.coverage === "incomplete" ? "warning" : "danger";
  return <Inspector title={t("m08Trace.title")} onClose={onClose}>
    <section className="workspace-section">
      <SectionHeading title={t("m03History.request")} />
      <dl className="detail-list">
        <Fact label={t("m03History.tool")}>{result.toolName}</Fact>
        <Fact label={t("m03History.requestId")}><code>{result.requestId}</code></Fact>
        <Fact label={t("m03History.runtimeId")}><code>{result.runtimeId}</code></Fact>
        {result.traceId && <Fact label={t("m08Trace.traceId")}><code>{result.traceId}</code></Fact>}
        <Fact label={t("m08Trace.availabilityLabel")}>{t(`m08Trace.availability.${result.availability}`)}</Fact>
        <Fact label={t("m08Trace.executorStarted")}>{result.executorStarted ? t("m08Trace.yes") : t("m08Trace.no")}</Fact>
        {result.policyDecision && <Fact label={t("m03History.policy")}>{result.policyDecision.toUpperCase()}</Fact>}
        {result.policyReason && <Fact label={t("m7.evidence.policyReason")}>{result.policyReason.toUpperCase()}</Fact>}
        {result.approvalState && <Fact label={t("m03History.approval")}>{t(`m03History.approvalState.${result.approvalState}`)}</Fact>}
        {result.approvalDecision && <Fact label={t("m7.evidence.approvalDecision")}>{result.approvalDecision.toUpperCase()}</Fact>}
        <Fact label={t("m03History.coverage")}><StatusBadge tone={tone}>{t(`m08Trace.coverage.${result.coverage}`)}</StatusBadge></Fact>
        {result.failureStage && <Fact label={t("m08Trace.failureStage")}>{t(`m08Trace.stage.${result.failureStage}`)}</Fact>}
        {result.slowestMeasuredSpan && <Fact label={t("m08Trace.slowestSpan")}>{t(`m08Trace.stage.${result.slowestMeasuredSpan}`)}</Fact>}
      </dl>
    </section>
    {result.spans.length === 0
      ? <InlineFeedback>{result.availability === "legacy_untraced" ? t("m08Trace.legacyUnavailable") : result.availability === "migration_required" ? t("m08Trace.migrationRequired") : t("m08Trace.noSpans")}</InlineFeedback>
      : <section className="workspace-section"><SectionHeading title={t("m08Trace.timeline")} /><ol className="history-timeline">{[...result.spans].sort((a, b) => a.spanSequence - b.spanSequence).map((span) => <li key={span.spanId}><strong>{t(`m08Trace.stage.${span.spanName}`)}</strong><span>{span.durationMs === undefined ? t("m08Trace.timingUnavailable") : `${span.durationMs} ms`}</span><span>{t(`m08Trace.status.${span.status}`)}</span></li>)}</ol></section>}
  </Inspector>;
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return <div><dt>{label}</dt><dd>{children}</dd></div>;
}
