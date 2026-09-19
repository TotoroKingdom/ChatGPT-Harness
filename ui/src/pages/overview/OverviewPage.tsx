import { useState } from "react";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { RuntimeOverview } from "./RuntimeOverview";

export function OverviewPage() {
  const data = useHarnessData();
  const [message, setMessage] = useState("");
  const [actionError, setActionError] = useState("");
  const canStart = (data.overall === "stopped" || data.overall === "error") && Boolean(data.eligibility?.canStart);
  const run = async (fn: () => Promise<{ ok: boolean; error?: { message: string } }>) => {
    setActionError("");
    setMessage("");
    const result = await fn();
    if (!result.ok) setActionError(result.error?.message ?? "Lifecycle action failed. Check the local control plane.");
    else setMessage("Lifecycle action completed.");
  };
  return (
    <div className="page-stack">
      <Card className="hero-card">
        <div><div className="eyebrow">Harness status</div><div className="hero-status"><span className="hero-orb" /><div><h2>Harness is {data.overall}</h2><p>Runtime, MCP, and Tunnel readiness are owned by the local control plane.</p></div></div></div>
        <div className="hero-actions">{data.overall === "stopped" || data.overall === "error" ? <button className="button button-primary" onClick={() => void run(data.startHarness)} disabled={!canStart}>Start Harness</button> : <button className="button button-secondary" disabled={data.overall === "stopping" || data.overall === "starting"} onClick={() => void run(data.stopHarness)}>Stop Harness</button>}{data.restartRequired && <button className="button button-warning" onClick={() => void run(data.restartHarness)}>Restart Harness</button>}</div>
      </Card>
      {data.restartRequired && <div className="notice notice-warning"><strong>Restart Required</strong><span>Configuration has changed since the current Harness session started.</span></div>}{(data.overall === "stopped" || data.overall === "error") && data.eligibility && <div className={`notice ${data.eligibility.canStart ? "notice-success" : "notice-warning"}`}><strong>{data.eligibility.status}</strong><span>{data.eligibility.message}</span></div>}
      {actionError && <div className="notice notice-danger">{actionError}</div>}{message && <div className="notice notice-success">{message}</div>}
      <Card><SectionHeading eyebrow="Launcher component state" title="Harness services" />
        <div className="counter-list">{(["runtime", "tunnel", "mcp"] as const).map((component) =>
          <div key={component}><span>{component === "mcp" ? "MCP" : component === "runtime" ? "Runtime" : "Tunnel"}</span><StatusBadge>{data.components[component]}</StatusBadge></div>
        )}</div>
      </Card>
      <RuntimeOverview />
    </div>
  );
}
