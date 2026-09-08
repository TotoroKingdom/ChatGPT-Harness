import { useCallback } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";

export function ToolsPage() {
  const { client } = useHarnessData();
  const read = useCallback(() => client.runtimeGetEffectiveTools(), [client]);
  const { state, refresh } = useControlRead(read);
  return <div className="page-stack">
    <SectionHeading title="Effective Tools" description="Availability and decisions are reported by Runtime, never inferred by this UI. ASK is not permission to execute."
      action={<button className="button button-secondary" onClick={refresh}>Refresh tools</button>} />
    <ControlReadView state={state}>{(tools) => tools.length === 0
      ? <Card><strong>No tools reported</strong><p className="muted">The Control Plane returned an empty catalog, not a readiness assertion.</p></Card>
      : <Card><div className="observation-table-wrap"><table className="observation-table">
        <caption className="muted">Read-only catalog snapshot · refresh for current observations</caption>
        <thead><tr><th scope="col">Tool name</th><th scope="col">Availability</th><th scope="col">Decision</th><th scope="col">Reason</th></tr></thead>
        <tbody>{tools.map((tool) => <tr key={tool.name}>
          <td className="mono">{tool.name}</td><td>{tool.availability}</td><td>{tool.decision.toUpperCase()}</td><td>Not provided by Control Plane</td>
        </tr>)}</tbody>
      </table></div></Card>}
    </ControlReadView>
  </div>;
}
