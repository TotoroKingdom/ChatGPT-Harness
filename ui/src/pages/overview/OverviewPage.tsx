import { useState } from "react";
import type { DiagnosticRun } from "@protocol/v1";
import { Card, SectionHeading, Stat, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { AsyncStateView } from "@/components/status/AsyncStates";

const tone = (value: string): "success" | "warning" | "danger" | "neutral" =>
  value === "ready" || value === "pass" ? "success" : value === "starting" || value === "stopping" || value === "degraded" || value === "warning" ? "warning" : value === "error" || value === "fail" ? "danger" : "neutral";

export function OverviewPage() {
  const data = useHarnessData();
  const [message, setMessage] = useState("");
  const [actionError, setActionError] = useState("");
  const [diagnostics, setDiagnostics] = useState<DiagnosticRun | null>(data.lastDiagnostics);
  const run = async (fn: () => Promise<{ ok: boolean; error?: { message: string } }>) => {
    setActionError("");
    const result = await fn();
    if (!result.ok) setActionError(result.error?.message ?? "Action failed");
    else setMessage("Lifecycle action completed.");
  };
  const runDiagnostics = async () => {
    setActionError("");
    try {
      setDiagnostics(await data.runDiagnostics());
      setMessage("Diagnostics completed.");
    } catch (error) {
      setActionError(error instanceof Error ? error.message : "Diagnostics could not run.");
    }
  };
  return (
    <div className="page-stack">
      <Card className="hero-card"><div><div className="eyebrow">Harness status</div><div className="hero-status"><span className="hero-orb" /><div><h2>{data.overall[0].toUpperCase() + data.overall.slice(1)}</h2><p>{data.overall === "ready" ? "All M2 mock components are ready." : "Local control-plane lifecycle is available."}</p></div></div></div><div className="hero-actions">{data.overall === "stopped" ? <button className="button button-primary" onClick={() => void run(data.startHarness)}>Start Harness</button> : <button className="button button-secondary" disabled={data.overall === "stopping" || data.overall === "starting"} onClick={() => void run(data.stopHarness)}>Stop Harness</button>}{data.restartRequired && <button className="button button-warning" onClick={() => void run(data.restartHarness)}>Restart Harness</button>}</div></Card>
      {data.restartRequired && <div className="notice notice-warning"><strong>Restart Required</strong><span>Configuration has changed since the current Harness session started.</span></div>}
      {actionError && <div className="notice notice-danger">{actionError}</div>}{message && <div className="notice notice-success">{message}</div>}
      <div className="stats-grid"><Stat label="Overall" value={<StatusBadge tone={tone(data.overall)}>{data.overall}</StatusBadge>} /><Stat label="Config revision" value={data.config?.configRevision?.toString() ?? "—"} /><Stat label="Workspace" value={data.config?.workspace.rootPath ? "Configured" : "Setup required"} /><Stat label="Security" value={<StatusBadge tone={data.config?.security.mode === "developer_test" ? "warning" : "success"}>{data.config?.security.mode === "developer_test" ? "Developer/Test" : "Normal"}</StatusBadge>} /></div>
      <div className="component-grid">{(["runtime", "tunnel", "mcp"] as const).map((key) => <Card key={key} className="component-card"><SectionHeading eyebrow="Independent state" title={key.toUpperCase()} /><StatusBadge tone={tone(data.components[key])}>{data.components[key]}</StatusBadge><p className="muted">{key === "runtime" ? "M0 Runtime data with M2 mock lifecycle." : key === "tunnel" ? "Located only; real startup is deferred to M7." : "Reachability unavailable until Runtime integration."}</p></Card>)}</div>
      <Card><SectionHeading eyebrow="Diagnostics" title="Local checks" action={<button className="button button-secondary" onClick={() => void runDiagnostics()}>Run Diagnostics</button>} />{diagnostics ? <div className="diagnostic-list"><div className="diagnostic-summary"><StatusBadge tone={tone(diagnostics.overallStatus)}>{diagnostics.overallStatus}</StatusBadge><span>{diagnostics.checks.length} checks · {diagnostics.finishedAt}</span></div>{diagnostics.checks.map((check) => <div className="diagnostic-row" key={check.id}><StatusBadge tone={tone(check.status)}>{check.status}</StatusBadge><div><strong>{check.id}</strong><p>{check.summary}</p>{check.remediation && <small>{check.remediation}</small>}</div></div>)}</div> : <p className="muted">No local diagnostic run yet.</p>}</Card>
      <Card><SectionHeading eyebrow="Runtime snapshot" title="Active session" /><AsyncStateView state={data.runtime}>{runtime => <dl className="detail-list"><div><dt>Runtime ID</dt><dd className="mono">{runtime.runtimeId}</dd></div><div><dt>Active revision</dt><dd>{runtime.configRevision.toString()}</dd></div><div><dt>Active workspace</dt><dd className="mono">{runtime.activeWorkspace || "Not active"}</dd></div></dl>}</AsyncStateView></Card>
    </div>
  );
}