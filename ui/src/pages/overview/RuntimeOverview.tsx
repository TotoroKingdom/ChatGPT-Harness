import { useCallback } from "react";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { useControlRead } from "@/app/providers/useControlRead";
import { Card, SectionHeading, Stat } from "@/components/common/primitives";
import { ControlReadView } from "@/components/status/ControlReadView";
import { Trans, useTranslation } from "react-i18next";
import { runtimeReadinessKey } from "@/i18n/presentation";

export function RuntimeOverview() {
  const { t } = useTranslation();
  const { client } = useHarnessData();
  const read = useCallback(() => client.runtimeGetDiagnostics(), [client]);
  const { state, refresh } = useControlRead(read);
  return <section aria-label={t("overview.runtimeTitle")} className="page-stack">
    <SectionHeading title={t("overview.runtimeTitle")} description={t("overview.diagnosticsHelp")}
      action={<button className="button button-secondary" onClick={refresh}>{t("overview.refresh")}</button>} />
    <ControlReadView state={state}>{(data) => <Card>
      <p className="muted"><Trans i18nKey="overview.snapshot" values={{ timestamp: data.capturedAt }} components={{ time: <time dateTime={data.capturedAt} /> }} /></p>
      <div className="stats-grid">
        <Stat label={t("overview.readiness")} value={t(runtimeReadinessKey(data.runtime.readiness))} />
        <Stat label={t("overview.workspaceSummary")} value={t(data.security.workspaceValidation === "validated_at_initialization" ? "overview.validated" : "overview.notInitialized")} hint={t("overview.workspaceHint")} />
        <Stat label={t("overview.policyStatus")} value={t(data.security.policyEngineLoaded ? "overview.engineLoaded" : "overview.engineNotLoaded")} hint={t("overview.policyHint")} />
        <Stat label={t("overview.toolSummary")} value={t("overview.toolCounts", { effective: data.tools.effectiveCount, declared: data.tools.declaredCount })} />
      </div>
    </Card>}</ControlReadView>
    <p className="muted">{t("overview.externalReadiness")}</p>
  </section>;
}
