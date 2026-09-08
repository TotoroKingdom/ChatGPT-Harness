import { useCallback } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";

export function ActivityPage() {
  const { client } = useHarnessData();
  const read = useCallback(() => client.runtimeGetActivitySnapshot(100), [client]);
  const { state, refresh } = useControlRead(read);
  return <div className="page-stack">
    <SectionHeading title="Activity Feed" description="Latest 100 retained entries. Newest received first, not sorted by timestamp. Manual snapshot, not a live stream."
      action={<button className="button button-secondary" onClick={refresh}>Refresh activity</button>} />
    <ControlReadView state={state}>{(entries) => entries.length === 0
      ? <Card><strong>No retained activity</strong><p className="muted">An empty snapshot does not establish readiness or mean no requests have ever occurred.</p></Card>
      : <Card><div className="observation-table-wrap"><table className="observation-table">
        <caption className="muted">Latest activity · reception order</caption>
        <thead><tr><th scope="col">Activity</th><th scope="col">Timestamp (UTC)</th><th scope="col">Tool</th><th scope="col">Decision</th><th scope="col">Outcome</th></tr></thead>
        <tbody>{[...entries].reverse().map((entry, index) => <tr key={index}>
          <td>{entry.eventType}</td>
          <td>{entry.timestamp === null ? "Not recorded" : <time dateTime={entry.timestamp}>{entry.timestamp}</time>}</td>
          <td>{entry.toolName ?? "Not recorded"}</td>
          <td>{entry.decisionSummary === null ? "Not evaluated" : entry.decisionSummary.toUpperCase()}</td>
          <td>{entry.outcomeSummary}</td>
        </tr>)}</tbody>
      </table></div></Card>}
    </ControlReadView>
  </div>;
}
