import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { ActivityEntry } from "@/services/harness/control-read";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import type { Navigate } from "@/app/router/routes";
import {
  Button,
  Card,
  Disclosure,
  InlineFeedback,
  Inspector,
  Row,
  Segmented,
  Select,
  SectionHeading,
  StatusBadge,
} from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";
import {
  activityCategoryKey,
  activityEventKey,
  activityFailureCategoryKey,
  activityLifecycleStateKey,
  approvalDecisionKey,
  errorCodeKey,
  policyDecisionKey,
  policyReasonKey,
} from "@/i18n/presentation";
import {
  activityOutcomeKey,
  activityOutcomeKind,
  activityOutcomeTone,
  activityDuration,
  activityRows,
  activityTarget,
  activityTools,
  filterActivityRows,
  reconcileActivitySelection,
  type ActivityFilter,
  type ActivityOutcomeKind,
  type ActivityRow,
  type ActivitySelection,
} from "./activity-feed";
import { HistoryPanel } from "./HistoryPanel";

type Snapshot = { rows: ActivityRow[] };

export function ActivityPage({ onNavigate }: { onNavigate?: Navigate } = {}) {
  const { t } = useTranslation();
  const { client, config, approvals } = useHarnessData();
  const read = useCallback(() => client.runtimeGetActivitySnapshot(100), [client]);
  const { state, refresh } = useControlRead(read);
  const snapshotId = useRef(0);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [selection, setSelection] = useState<ActivitySelection | null>(null);
  const [outcomeFilter, setOutcomeFilter] = useState<Exclude<ActivityFilter, `tool:${string}`>>("all");
  const [toolFilter, setToolFilter] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const [readError, setReadError] = useState<{ code: string } | null>(null);
  const [tab, setTab] = useState<"live" | "history">("live");

  useEffect(() => {
    if (state.status === "loading") {
      if (snapshot) setRefreshing(true);
      return;
    }
    if (state.status === "error") {
      setRefreshing(false);
      setReadError(state.error);
      return;
    }
    const rows = activityRows(state.data, ++snapshotId.current);
    setSnapshot({ rows });
    setSelection((current) => reconcileActivitySelection(current, rows));
    setReadError(null);
    setRefreshing(false);
  }, [state]);

  const rows = snapshot?.rows ?? [];
  const outcomeRows = useMemo(() => filterActivityRows(rows, outcomeFilter), [outcomeFilter, rows]);
  const visibleRows = useMemo(() => toolFilter ? filterActivityRows(outcomeRows, `tool:${toolFilter}`) : outcomeRows, [outcomeRows, toolFilter]);
  const tools = useMemo(() => activityTools(rows), [rows]);
  const selectedRow = selection ? rows.find((row) => row.rowKey === selection.rowKey) : undefined;

  useEffect(() => {
    if (selection && !visibleRows.some((row) => row.rowKey === selection.rowKey)) setSelection(null);
  }, [selection, visibleRows]);

  const onRefresh = () => {
    setRefreshing(true);
    refresh();
  };

  return <div className="page-stack">
    <SectionHeading
      title={t("activity.feed")}
      description={t("activity.help")}
      action={tab === "live" ? <Button variant="secondary" onClick={onRefresh} disabled={refreshing}>{t("activity.refresh")}</Button> : undefined}
    />
    <Segmented
      value={tab}
      label={t("m03History.view")}
      options={[
        { value: "live", label: t("m03History.live") },
        { value: "history", label: t("m03History.history") },
      ]}
      onChange={setTab}
    />
    {tab === "history"
      ? <HistoryPanel />
      : <>
        {refreshing && <InlineFeedback>{t("m8Activity.refreshing")}</InlineFeedback>}
        {readError && snapshot && <div className="notice notice-warning" role="status">
          <strong>{t("m8Activity.stale")}</strong>
          <span>{t("m8Activity.staleHelp")} {t(errorCodeKey(readError.code))}</span>
        </div>}
        {!snapshot
          ? <ControlReadView state={state}>{() => null}</ControlReadView>
          : <ActivityFeed
            rows={rows}
            visibleRows={visibleRows}
            tools={tools}
            outcomeFilter={outcomeFilter}
            onOutcomeFilter={setOutcomeFilter}
            toolFilter={toolFilter}
            onToolFilter={setToolFilter}
            onInspect={(row) => setSelection({ rowKey: row.rowKey, ...(row.stableEventId ? { eventId: row.stableEventId } : {}) })}
            selectedRow={selectedRow}
            onClose={() => setSelection(null)}
            onNavigate={onNavigate}
            config={config}
            approvals={approvals}
            t={t}
          />}
      </>}
  </div>;
}

type ActivityFeedProps = {
  rows: ActivityRow[];
  visibleRows: ActivityRow[];
  tools: string[];
  outcomeFilter: Exclude<ActivityFilter, `tool:${string}`>;
  onOutcomeFilter: (filter: Exclude<ActivityFilter, `tool:${string}`>) => void;
  toolFilter: string;
  onToolFilter: (tool: string) => void;
  onInspect: (row: ActivityRow) => void;
  selectedRow?: ActivityRow;
  onClose: () => void;
  onNavigate?: Navigate;
  config: ReturnType<typeof useHarnessData>["config"];
  approvals: ReturnType<typeof useHarnessData>["approvals"];
  t: (key: string, options?: Record<string, unknown>) => string;
};

function ActivityFeed({ rows, visibleRows, tools, outcomeFilter, onOutcomeFilter, toolFilter, onToolFilter, onInspect, selectedRow, onClose, onNavigate, config, approvals, t }: ActivityFeedProps) {
  return <>
    <Card className="activity-card">
      <div className="activity-filter" role="toolbar" aria-label={t("m8Activity.toolFilter")}>
        <span className="muted">{t("m8Activity.scope")}</span>
        <Segmented
          value={outcomeFilter}
          options={[
            { value: "all", label: t("m8Activity.all") },
            { value: "success", label: t("m8Activity.success") },
            { value: "attention", label: t("m8Activity.attention") },
          ]}
          onChange={(value) => onOutcomeFilter(value as Exclude<ActivityFilter, `tool:${string}`>)}
          label={t("activity.outcome")}
        />
        {tools.length > 0 && <Select
          label={t("m8Activity.toolFilter")}
          value={toolFilter}
          onChange={(event) => onToolFilter(event.target.value)}
        >
          <option value="">{t("m8Activity.all")}</option>
          {tools.map((tool) => <option key={tool} value={tool}>{tool}</option>)}
        </Select>}
        <span className="catalog-count">{visibleRows.length}/{rows.length}</span>
      </div>
      {rows.length === 0
        ? <div className="empty-inline"><strong>{t("m8Activity.empty")}</strong><p>{t("m8Activity.emptyHelp")}</p></div>
        : visibleRows.length === 0
          ? <div className="empty-inline"><p>{t("m8Activity.noMatches")}</p></div>
          : <div aria-label={t("activity.feed")}>{[...visibleRows].reverse().map((row) => <ActivityRowView key={row.rowKey} row={row} selected={selectedRow?.rowKey === row.rowKey} onInspect={() => onInspect(row)} t={t} />)}</div>}
    </Card>
    {selectedRow && <ActivityInspector row={selectedRow} onClose={onClose} onNavigate={onNavigate} config={config} approvals={approvals} t={t} />}
  </>;
}

function ActivityRowView({ row, selected, onInspect, t }: { row: ActivityRow; selected: boolean; onInspect: () => void; t: ActivityFeedProps["t"] }) {
  const { entry } = row;
  const kind = activityOutcomeKind(entry);
  const target = activityTarget(entry);
  const timestamp = entry.timestamp ?? t("m8Activity.notRecorded");
  const event = t(activityEventKey(entry.eventType));
  const tool = entry.toolName === null ? t("m8Activity.noTool") : entry.toolName ?? t("m8Activity.notRecorded");
  const duration = activityDuration(entry);
  const targetText = target
    ? <><span>{t(target.kind === "command" ? "m8Activity.command" : "m8Activity.target")}: </span><code>{target.value}</code></>
    : entry.toolName === null ? t("m8Activity.noTool") : t("m8Activity.notRecorded");
  return <article className="activity-row" data-testid={`activity-row-${row.rowKey}`}>
    <Row selected={selected} onInspect={onInspect} title={<>
      <span className="activity-type">
        <span className={`event-marker ${activityOutcomeTone(kind)}`} aria-hidden="true" />
        {entry.timestamp ? <time dateTime={entry.timestamp}>{timestamp}</time> : <span>{timestamp}</span>}
        <strong>{tool}</strong>
      </span>
      <span className="row-summary">{targetText}</span>
      <span className="row-summary"><span>{event}</span> · <StatusBadge tone={activityOutcomeTone(kind)}>{t(activityOutcomeKey(kind))}</StatusBadge>{duration !== undefined && <> · {duration}</>}</span>
    </>} />
  </article>;
}

function ActivityInspector({ row, onClose, onNavigate, config, approvals, t }: {
  row: ActivityRow;
  onClose: () => void;
  onNavigate?: Navigate;
  config: ActivityFeedProps["config"];
  approvals: ActivityFeedProps["approvals"];
  t: ActivityFeedProps["t"];
}) {
  const { entry } = row;
  const kind = activityOutcomeKind(entry);
  const target = activityTarget(entry);
  const duration = activityDuration(entry);
  const outcome = t(activityOutcomeKey(kind));
  const event = t(activityEventKey(entry.eventType));
  const currentRules = config?.policies?.userRules ?? [];
  const currentApprovals = approvals && (approvals.status === "success" || approvals.status === "empty") ? approvals.data.items : [];
  const matchedRules = entry.matchedRuleIds.map((id) => currentRules.find((rule) => rule.id === id)).filter((rule): rule is (typeof currentRules)[number] => Boolean(rule));
  const currentApproval = entry.approvalId ? currentApprovals.find((approval) => approval.requestId === entry.approvalId) : undefined;

  return <Inspector title={t("m8Activity.inspectorTitle")} onClose={onClose}>
    <p className="muted">{event}{entry.toolName === null ? ` · ${t("m8Activity.noTool")}` : ` · ${entry.toolName ?? t("m8Activity.notRecorded")}`}</p>
    <InspectorSection title={t("m8Activity.what")}>
      <p>{event}{target ? <> · {t(target.kind === "command" ? "m8Activity.command" : "m8Activity.target")}: <code>{target.value}</code></> : <> · {entry.toolName === null ? t("m8Activity.noTool") : t("m8Activity.notRecorded")}</>}</p>
    </InspectorSection>
    <InspectorSection title={t("m8Activity.why")}>
      <p>{whyText(entry, kind, t)}</p>
      <Facts entry={entry} t={t} />
    </InspectorSection>
    <InspectorSection title={t("m8Activity.result")}>
      <p><StatusBadge tone={activityOutcomeTone(kind)}>{outcome}</StatusBadge></p>
      {duration !== undefined && <p>{t("m7.evidence.duration")}: <code>{duration}</code></p>}
      {entry.resultEvidence && <pre>{JSON.stringify(entry.resultEvidence, null, 2)}</pre>}
      {!entry.resultEvidence && duration === undefined && <p className="muted">{t("m8Activity.notRecorded")}</p>}
    </InspectorSection>
    <InspectorSection title={t("m8Activity.next")}>
      <p>{t(`m8Activity.nextStep.${kind === "failed" ? "execution_failed" : kind}`)}</p>
    </InspectorSection>
    {(entry.matchedRuleIds.length > 0 || entry.approvalId) && <InspectorSection title={t("m8Activity.event")}>
      {entry.matchedRuleIds.length > 0 && <div className="activity-links">
        <p>{t("m8Activity.historicalRule")}</p>
        {matchedRules.length > 0 && onNavigate
          ? matchedRules.map((rule) => <Button key={rule.id} variant="quiet" onClick={() => onNavigate("policies", { objectId: rule.id })}>{t("m8Activity.openRule", { id: rule.id })}</Button>)
          : <p className="muted">{t("m8Activity.noCurrentRule")}</p>}
      </div>}
      {entry.approvalId && <div className="activity-links">
        <p>{t("m8Activity.historicalApproval")}</p>
        {currentApproval && onNavigate
          ? <Button variant="quiet" onClick={() => onNavigate("approvals", { objectId: currentApproval.requestId })}>{t("m8Activity.openApproval", { id: currentApproval.requestId })}</Button>
          : <p className="muted">{t("m8Activity.noCurrentApproval")}</p>}
      </div>}
    </InspectorSection>}
    <Disclosure title={t("m8Activity.advanced")}>
      <pre aria-label={t("m8Activity.advanced")}>{JSON.stringify(entry, null, 2)}</pre>
    </Disclosure>
  </Inspector>;
}

function InspectorSection({ title, children }: { title: string; children: ReactNode }) {
  return <section className="workspace-section"><SectionHeading title={title} />{children}</section>;
}

function Facts({ entry, t }: { entry: ActivityEntry; t: ActivityFeedProps["t"] }) {
  const facts: Array<[string, ReactNode]> = [
    [t("m8Activity.decision"), entry.decisionSummary === null ? t("m8Activity.notEvaluated") : t(policyDecisionKey(entry.decisionSummary))],
    [t("m8Activity.category"), entry.category ? t(activityCategoryKey(entry.category)) : t("m8Activity.notRecorded")],
    [t("m8Activity.lifecycle"), entry.lifecycleState ? t(activityLifecycleStateKey(entry.lifecycleState)) : t("m8Activity.notRecorded")],
    [t("m8Activity.request"), entry.requestId ?? t("m8Activity.notRecorded")],
    [t("m8Activity.event"), entry.eventId ?? t("m8Activity.notRecorded")],
  ];
  if (entry.policyReason) facts.push([t("m7.evidence.policyReason"), t(policyReasonKey(entry.policyReason))]);
  if (entry.failureCategory) facts.push([t("m7.evidence.failure"), t(activityFailureCategoryKey(entry.failureCategory))]);
  if (entry.approvalDecision) facts.push([t("m7.evidence.approvalDecision"), t(approvalDecisionKey(entry.approvalDecision))]);
  return <dl className="detail-list">{facts.map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>;
}

function whyText(entry: ActivityEntry, kind: ActivityOutcomeKind, t: ActivityFeedProps["t"]): string {
  if (kind === "policy_denied") return entry.policyReason ? t(policyReasonKey(entry.policyReason)) : t("m8Activity.outcome.policy_denied");
  if (kind === "approval_denied") return t("m8Activity.outcome.approval_denied");
  if (entry.policyReason) return t(policyReasonKey(entry.policyReason));
  if (entry.failureCategory) return t(activityFailureCategoryKey(entry.failureCategory));
  if (entry.decisionSummary) return t(policyDecisionKey(entry.decisionSummary));
  return t("m8Activity.notRecorded");
}
