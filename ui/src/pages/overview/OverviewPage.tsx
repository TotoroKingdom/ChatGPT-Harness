import { AsyncStateView } from "@/components/status/AsyncStates";
import { Card, SectionHeading, Stat, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";

const tone = (value: string): "success" | "warning" | "danger" | "neutral" => value === "ready" || value === "allow" ? "success" : value === "ask" || value === "degraded" || value === "initializing" ? "warning" : value === "error" || value === "deny" ? "danger" : "neutral";

export function OverviewPage() {
  const { runtime } = useHarnessData();
  return <div className="page-stack">
    <AsyncStateView state={runtime}>{(data) => <>
      <Card className="hero-card"><div><div className="eyebrow">Runtime status</div><div className="hero-status"><span className="hero-orb" /><div><h2>Mock Runtime is {data.readiness.replace("_", " ")}</h2><p>Read-only control-plane preview backed by the M0 contract.</p></div></div></div><div className="hero-actions"><button className="button button-primary" disabled>Start Harness</button><span>Available in M2</span></div></Card>
      <div className="stats-grid"><Stat label="Lifecycle" value={<StatusBadge tone={tone(data.lifecycle)}>{data.lifecycle}</StatusBadge>} /><Stat label="Readiness" value={<StatusBadge tone={tone(data.readiness)}>{data.readiness.replace("_", " ")}</StatusBadge>} /><Stat label="Exposed tools" value={data.exposedToolCatalog.length} hint="from Runtime catalog" /><Stat label="Pending approvals" value={data.pendingApprovals} hint="approval UI arrives in M8" /></div>
      <div className="overview-grid"><Card><SectionHeading eyebrow="Observed state" title="Runtime details" /><dl className="detail-list"><div><dt>Runtime ID</dt><dd className="mono">{data.runtimeId}</dd></div><div><dt>Started at</dt><dd>{data.startedAt}</dd></div><div><dt>Active workspace</dt><dd className="mono">{data.activeWorkspace}</dd></div><div><dt>Security mode</dt><dd><StatusBadge tone="neutral">{data.securityMode}</StatusBadge></dd></div></dl></Card><Card><SectionHeading eyebrow="Live counters" title="Execution activity" /><div className="counter-list"><div><span>Active requests</span><strong>{data.activeRequests}</strong></div><div><span>Running executions</span><strong>{data.runningExecutions}</strong></div><div><span>Catalog entries</span><strong>{data.exposedToolCatalog.length}</strong></div></div></Card></div>
      {data.lastError && <Card className="notice notice-danger"><StatusBadge tone="danger">Last error</StatusBadge><div><strong>{data.lastError.message}</strong><span className="muted">{data.lastError.code}</span></div></Card>}
    </>}</AsyncStateView>
    <div className="secondary-actions"><button className="button button-secondary" disabled>Stop Harness</button><button className="button button-secondary" disabled>Run Diagnostics</button><span>Lifecycle controls become available in M2.</span></div>
  </div>;
}
