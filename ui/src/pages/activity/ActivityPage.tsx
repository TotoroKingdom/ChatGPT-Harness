import { useCallback } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";
import { useTranslation } from "react-i18next";
import { activityEventKey, activityOutcomeKey, activityFailureCategoryKey, policyDecisionKey } from "@/i18n/presentation";

export function ActivityPage() {
  const { t } = useTranslation();
  const { client } = useHarnessData();
  const read = useCallback(() => client.runtimeGetActivitySnapshot(100), [client]);
  const { state, refresh } = useControlRead(read);
  return <div className="page-stack">
    <SectionHeading title={t("activity.feed")} description={t("activity.help")}
      action={<button className="button button-secondary" onClick={refresh}>{t("activity.refresh")}</button>} />
    <ControlReadView state={state}>{(entries) => entries.length === 0
      ? <Card><strong>{t("activity.empty")}</strong><p className="muted">{t("activity.emptyHelp")}</p></Card>
      : <Card><div className="observation-table-wrap"><table className="observation-table">
        <caption className="muted">{t("activity.caption")}</caption>
        <thead><tr><th scope="col">{t("activity.title")}</th><th scope="col">{t("activity.timestamp")}</th><th scope="col">{t("activity.tool")}</th><th scope="col">{t("activity.decision")}</th><th scope="col">{t("activity.outcome")}</th></tr></thead>
        <tbody>{[...entries].reverse().map((entry, index) => <tr key={index}>
          <td>{t(activityEventKey(entry.eventType))}</td>
          <td>{entry.timestamp === null ? t("activity.notRecorded") : <time dateTime={entry.timestamp}>{entry.timestamp}</time>}</td>
          <td>{entry.toolName ?? t("activity.notRecorded")}</td>
          <td>{entry.decisionSummary === null ? t("activity.notEvaluated") : t(policyDecisionKey(entry.decisionSummary))}</td>
          <td>{t(activityOutcomeKey(entry.outcomeSummary))}{entry.failureCategory && <><br /><small>{t(activityFailureCategoryKey(entry.failureCategory))}</small></>}</td>
        </tr>)}</tbody>
      </table></div></Card>}
    </ControlReadView>
  </div>;
}
