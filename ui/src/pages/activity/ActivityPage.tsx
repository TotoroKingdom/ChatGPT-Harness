import type { ActivityEntry } from "@/services/harness/control-read";
import { Fragment, useCallback, useState } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import { Card, SectionHeading } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";
import { useTranslation } from "react-i18next";
import { activityEventKey, activityOutcomeKey, activityFailureCategoryKey, policyDecisionKey, policyReasonKey, approvalDecisionKey } from "@/i18n/presentation";

export function ActivityPage() {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<number | null>(null);
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
        <thead><tr><th scope="col">{t("activity.title")}</th><th scope="col">{t("activity.timestamp")}</th><th scope="col">{t("activity.tool")}</th><th scope="col">{t("activity.decision")}</th><th scope="col">{t("activity.outcome")}</th><th scope="col">{t("m7.evidence.detail")}</th></tr></thead>
        <tbody>{[...entries].reverse().map((entry, index) => <Fragment key={entry.eventId ?? index}><tr>
          <td>{t(activityEventKey(entry.eventType))}</td>
          <td>{entry.timestamp === null ? t("activity.notRecorded") : <time dateTime={entry.timestamp}>{entry.timestamp}</time>}</td>
          <td>{entry.toolName ?? t("activity.notRecorded")}</td>
          <td>{entry.decisionSummary === null ? t("activity.notEvaluated") : t(policyDecisionKey(entry.decisionSummary))}</td>
          <td>{t(activityOutcomeKey(entry.outcomeSummary))}{entry.failureCategory && <><br /><small>{t(activityFailureCategoryKey(entry.failureCategory))}</small></>}</td>
          <td><button className="button button-secondary" aria-expanded={expanded === index} onClick={() => setExpanded(expanded === index ? null : index)}>{t("m7.evidence.detail")}</button></td>
        </tr>{expanded === index && <tr><td colSpan={6}><ActivityDetail entry={entry} /></td></tr>}</Fragment>)}</tbody>
      </table></div></Card>}
    </ControlReadView>
  </div>;
}

function ActivityDetail({ entry }: { entry: ActivityEntry }) {
  const { t } = useTranslation();
  const missing = t("activity.notRecorded");
  const facts: Array<[string, string | number]> = [
    ["request", entry.requestId ?? missing], ["event", entry.eventId ?? missing],
    ["arguments", entry.sanitizedArguments ? JSON.stringify(entry.sanitizedArguments) : missing],
    ["enabled", entry.toolEnabled === undefined ? missing : t(entry.toolEnabled ? "m7.evidence.yes" : "m7.evidence.no")],
    ["capability", entry.capability ?? missing],
    ["policyReason", entry.policyReason ? t(policyReasonKey(entry.policyReason)) : missing],
    ["matched", entry.matchedRuleIds.length ? entry.matchedRuleIds.join(", ") : missing],
    ["approval", entry.approvalId ?? missing],
    ["approvalDecision", entry.approvalDecision ? t(approvalDecisionKey(entry.approvalDecision)) : missing],
    ["result", entry.resultEvidence ? JSON.stringify(entry.resultEvidence) : missing],
    ["duration", entry.durationMs ?? missing],
    ["failure", entry.failureCategory ? t(activityFailureCategoryKey(entry.failureCategory)) : missing],
    ["bash", entry.bash ? JSON.stringify(entry.bash) : t("m7.evidence.notApplicable")],
  ];
  return <section aria-label={t("m7.evidence.detail")}><dl>{facts.map(([key, value]) => <Fragment key={key}><dt>{t(`m7.evidence.${key}`)}</dt><dd><code>{value}</code></dd></Fragment>)}</dl>
    {entry.failureCategory && <p>{t("m7.remediation.execution_failure")}</p>}
  </section>;
}
