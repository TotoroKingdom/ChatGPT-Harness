import { useCallback } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import { Card, SectionHeading, Stat } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";

export function RuntimeOverview() {
  const { client } = useHarnessData();
  const read = useCallback(() => client.runtimeGetDiagnostics(), [client]);
  const { state, refresh } = useControlRead(read);
  return <section aria-label="Runtime Overview" className="page-stack">
    <SectionHeading title="Runtime Overview" description="Read-only diagnostics. Runtime readiness does not establish external MCP readiness."
      action={<button className="button button-secondary" onClick={refresh}>Refresh overview</button>} />
    <ControlReadView state={state}>{(data) => <Card>
      <p className="muted">Snapshot captured at <time dateTime={data.capturedAt}>{data.capturedAt}</time>. Refresh for a new observation.</p>
      <div className="stats-grid">
        <Stat label="Runtime readiness" value={data.runtime.readiness} />
        <Stat label="Workspace summary" value={data.security.workspaceValidation === "validated_at_initialization" ? "Validated at initialization" : "Not initialized"} hint="Initialization evidence only; no filesystem re-check." />
        <Stat label="Policy status" value={data.security.policyEngineLoaded ? "Engine loaded" : "Engine not loaded"} hint="Construction observed, not rule validity or permission to execute." />
        <Stat label="Tool summary" value={data.tools.effectiveCount + " directly available / " + data.tools.declaredCount + " declared"} />
      </div>
    </Card>}</ControlReadView>
    <p className="muted">M7-C: BLOCKED · External MCP readiness: NOT ESTABLISHED</p>
  </section>;
}
