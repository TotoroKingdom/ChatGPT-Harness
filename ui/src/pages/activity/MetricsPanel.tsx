import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { MetricsQueryResult, MetricsSampleStatus, TraceSpanName } from "@protocol/v1";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { Button, Card, InlineFeedback, Select, StatusBadge } from "@/components/common/primitives";
import { errorCodeKey } from "@/i18n/presentation";

type Range = "24h" | "7d" | "30d";

const rangeHours: Record<Range, number> = { "24h": 24, "7d": 24 * 7, "30d": 24 * 30 };

function dateRange(range: Range) {
  const toUtc = new Date().toISOString();
  const fromUtc = new Date(Date.now() - rangeHours[range] * 60 * 60 * 1000).toISOString();
  return { fromUtc, toUtc };
}

function metricStatus(status: MetricsSampleStatus, t: (key: string) => string) {
  return t(`m08Metrics.sample.${status}`);
}

function milliseconds(value: number | undefined, t: (key: string) => string) {
  return value === undefined ? t("m08Metrics.unavailable") : `${Math.round(value)} ms`;
}

export function MetricsPanel() {
  const { t } = useTranslation();
  const { client } = useHarnessData();
  const [range, setRange] = useState<Range>("24h");
  const [tool, setTool] = useState("");
  const [result, setResult] = useState<MetricsQueryResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestGeneration = useRef(0);

  const load = useCallback(async () => {
    const generation = ++requestGeneration.current;
    const query = { ...dateRange(range), ...(tool ? { tool } : {}) };
    setLoading(true);
    setError(null);
    try {
      const next = await client.metricsQuery(query);
      if (generation === requestGeneration.current) setResult(next);
    } catch (failure) {
      if (generation === requestGeneration.current) setError(typeof failure === "object" && failure !== null && "code" in failure ? String(failure.code) : "EVIDENCE_IO");
    } finally {
      if (generation === requestGeneration.current) setLoading(false);
    }
  }, [client, range, tool]);

  useEffect(() => { void load(); }, [load]);

  return <Card className="activity-card" aria-label={t("m08Metrics.title")}>
    <div className="filter-toolbar" role="toolbar" aria-label={t("m08Metrics.filters")}>
      <Select label={t("m08Metrics.timeRange")} value={range} onChange={(event) => setRange(event.target.value as Range)}>
        <option value="24h">{t("m03History.range.24h")}</option>
        <option value="7d">{t("m03History.range.7d")}</option>
        <option value="30d">{t("m03History.range.30d")}</option>
      </Select>
      <Select label={t("m08Metrics.tool")} value={tool} onChange={(event) => setTool(event.target.value)}>
        <option value="">{t("m03History.all")}</option>
        {(["read_file", "glob", "grep", "write", "edit", "bash"] as const).map((name) => <option key={name} value={name}>{name}</option>)}
      </Select>
      <Button variant="secondary" onClick={() => void load()} disabled={loading}>{loading ? t("m08Metrics.loading") : t("m08Metrics.refresh")}</Button>
    </div>
    {error && <div className="notice notice-warning" role="alert"><strong>{t("m08Metrics.error")}</strong> <span>{t(errorCodeKey(error))}</span></div>}
    {loading && !result && <InlineFeedback>{t("m08Metrics.loading")}</InlineFeedback>}
    {result && <MetricsResult result={result} t={t} />}
  </Card>;
}

function MetricsResult({ result, t }: { result: MetricsQueryResult; t: (key: string, options?: Record<string, unknown>) => string }) {
  const c = result.counts;
  const volume: Array<[string, number]> = [
    [t("m08Metrics.received"), c.receivedCalls], [t("m08Metrics.executionAttempts"), c.executionAttempts],
    [t("m08Metrics.incomplete"), c.incompleteCalls], [t("m08Metrics.unknown"), c.unknownCalls],
  ];
  const outcomes: Array<[string, number]> = [
    [t("m08Metrics.successful"), c.successfulCalls], [t("m08Metrics.failed"), c.failedCalls],
    [t("m08Metrics.denied"), c.deniedCalls], [t("m08Metrics.cancelled"), c.cancelledCalls],
    [t("m08Metrics.timedOut"), c.timedOutCalls], [t("m08Metrics.preExecutionFailed"), c.preExecutionFailedCalls],
    [t("m08Metrics.approvalExpired"), c.approvalExpiredCalls], [t("m08Metrics.approvalCancelled"), c.approvalCancelledCalls],
  ];
  const primaryDuration = result.execution.averageMs === undefined ? t("m08Metrics.unavailable") : `${Math.round(result.execution.averageMs)} ms`;
  return <>
    <div className="metrics-summary">
      <div className="metric-card"><span>{t("m08Metrics.received")}</span><strong>{c.receivedCalls}</strong><span>{t("m08Metrics.executionAttempts")}: {c.executionAttempts}</span></div>
      <div className="metric-card"><span>{t("m08Metrics.successRate")}</span><strong>{c.executionSuccessRate === undefined ? "—" : `${(c.executionSuccessRate * 100).toFixed(1)}%`}</strong><span>{t("m08Metrics.denominator", { numerator: c.executionSuccessNumerator, denominator: c.completedExecutionDenominator })}</span></div>
      <div className="metric-card"><span>{t("m08Metrics.executionDuration")}</span><strong>{primaryDuration}</strong><span>{t("m08Metrics.samples", { count: result.execution.sampleCount })}</span></div>
    </div>
    <div className="metrics-content page-stack">
      <section className="workspace-section">
        <h3>{t("m08Metrics.received")} / {t("m08Metrics.executionAttempts")}</h3>
        <dl className="detail-list metrics-counts">{volume.map(([label, value]) => <div key={label}><dt>{label}</dt><dd><strong>{value}</strong></dd></div>)}</dl>
      </section>
      <section className="workspace-section">
        <div className="section-heading"><div className="section-heading-copy"><h3>{t("m08Metrics.successRate")}</h3></div><StatusBadge tone={c.executionSuccessSampleStatus === "SUFFICIENT" ? "success" : c.executionSuccessSampleStatus === "NO_DATA" ? "neutral" : "warning"} visual={c.executionSuccessSampleStatus === "SUFFICIENT" ? "success" : c.executionSuccessSampleStatus === "NO_DATA" ? "unknown" : "warning"}>{metricStatus(c.executionSuccessSampleStatus, t)}</StatusBadge></div>
        <dl className="detail-list metrics-counts">{outcomes.map(([label, value]) => <div key={label}><dt>{label}</dt><dd><strong>{value}</strong></dd></div>)}</dl>
      </section>
      <section className="workspace-section">
        <h3>{t("m08Metrics.p95")}</h3>
        <dl className="detail-list metrics-durations">
          <Duration label={t("m08Metrics.executionDuration")} metric={result.execution} t={t} />
          <Duration label={t("m08Metrics.approvalWait")} metric={result.approvalWait} t={t} />
          <Duration label={t("m08Metrics.requestWall")} metric={result.requestWall} t={t} />
        </dl>
      </section>
      <section className="workspace-section">
        <h3>{t("m08Metrics.coverage")}</h3>
        <p>{t("m08Metrics.coverageSummary", { complete: result.coverage.completeRequestCount, incomplete: result.coverage.incompleteRequestCount, unknown: result.coverage.unknownRequestCount, legacy: result.coverage.legacyUntracedRequestCount })}</p>
        {(result.coverage.storeStatus.state !== "healthy" || result.coverage.dropCount > 0 || result.coverage.coverageUnknownSinceUtc) && <div className="notice notice-warning" role="status">
          <strong>{t("m08Metrics.coverageWarning")}</strong>
          <span>{t(`m03History.storeState.${result.coverage.storeStatus.state}`)} · {t("m08Metrics.dropped", { count: result.coverage.dropCount })}</span>
        </div>}
        {result.coverage.storeStatus.state === "healthy" && result.coverage.dropCount === 0 && !result.coverage.coverageUnknownSinceUtc && <p className="muted">{t("m03History.storeState.healthy")}</p>}
      </section>
    </div>
  </>;
}

function Duration({ label, metric, t }: { label: string; metric: MetricsQueryResult["execution"]; t: (key: string, options?: Record<string, unknown>) => string }) {
  return <div><dt>{label}</dt><dd>{t("m08Metrics.average")} <strong>{milliseconds(metric.averageMs, t)}</strong> · {t("m08Metrics.p95")} <strong>{milliseconds(metric.p95Ms, t)}</strong> · {t("m08Metrics.samples", { count: metric.sampleCount })} · {metricStatus(metric.sampleStatus, t)}</dd></div>;
}

export function traceStageName(name: TraceSpanName, t: (key: string) => string) {
  return t(`m08Trace.stage.${name}`);
}
