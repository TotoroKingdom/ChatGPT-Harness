import { useState } from "react";
import { Card, SectionHeading, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import { RuntimeOverview } from "./RuntimeOverview";
import { useTranslation } from "react-i18next";
import { componentStatusKey, errorCodeKey, overallStatusKey, startEligibilityKey } from "@/i18n/presentation";

export function OverviewPage() {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [message, setMessage] = useState("");
  const [actionError, setActionError] = useState("");
  const canStart = (data.overall === "stopped" || data.overall === "error") && Boolean(data.eligibility?.canStart);
  const tunnelReadiness = data.harness?.tunnelReadiness;
  const run = async (fn: () => Promise<{ ok: boolean; error?: { message: string; code?: string } }>) => {
    setActionError("");
    setMessage("");
    const result = await fn();
    if (!result.ok) setActionError(errorCodeKey(result.error));
    else setMessage("overview.actionCompleted");
  };
  return (
    <div className="page-stack">
      <Card className="hero-card">
        <div><div className="eyebrow">{t("overview.harnessStatus")}</div><div className="hero-status"><span className="hero-orb" /><div><h2>{t("overview.harnessIs", { status: t(overallStatusKey(data.overall)) })}</h2><p>{t("overview.authority")}</p></div></div></div>
        <div className="hero-actions">{data.overall === "stopped" || data.overall === "error" ? <button className="button button-primary" onClick={() => void run(data.startHarness)} disabled={!canStart}>{t("overview.start")}</button> : <button className="button button-secondary" disabled={data.overall === "stopping" || data.overall === "starting"} onClick={() => void run(data.stopHarness)}>{t("overview.stop")}</button>}{data.restartRequired && <button className="button button-warning" onClick={() => void run(data.restartHarness)}>{t("overview.restart")}</button>}</div>
      </Card>
      {data.restartRequired && <div className="notice notice-warning"><strong>{t("overview.restartRequired")}</strong><span>{t("overview.configChanged")}</span></div>}{(data.overall === "stopped" || data.overall === "error") && data.eligibility && <div className={`notice ${data.eligibility.canStart ? "notice-success" : "notice-warning"}`}><strong>{t(startEligibilityKey(data.eligibility.status))}</strong><span>{t(`overview.eligibility.${data.eligibility.status}`)}</span></div>}
      {actionError && <div className="notice notice-danger">{t(actionError)}</div>}{message && <div className="notice notice-success">{t(message)}</div>}
      {tunnelReadiness && !tunnelReadiness.controlPlaneConnected && (
        <div className="notice notice-warning">
          <strong>{data.harness?.lastError?.code ?? "CONTROL_PLANE_UNAVAILABLE"}</strong>
          <span>{t(errorCodeKey(data.harness?.lastError?.code ?? "CONTROL_PLANE_UNAVAILABLE"))}</span>
        </div>
      )}
      <Card><SectionHeading eyebrow={t("overview.componentState")} title={t("overview.services")} />
        <div className="counter-list">{(["runtime", "tunnel", "mcp"] as const).map((component) =>
          <div key={component}><span>{component === "mcp" ? "MCP" : component === "runtime" ? "Runtime" : "Tunnel"}</span><StatusBadge>{t(componentStatusKey(data.components[component]))}</StatusBadge></div>
        )}</div>
        {tunnelReadiness?.controlPlaneRoute && (
          <div className="local-facts">
            <div><span>{t("overview.route")}</span><code>{tunnelReadiness.controlPlaneRoute.routeMode}</code></div>
            <div><span>{t("overview.proxySource")}</span><code>{tunnelReadiness.controlPlaneRoute.proxySource || t("overview.none")}</code></div>
            {tunnelReadiness.controlPlaneRoute.proxyUrl && <div><span>{t("overview.proxyUrl")}</span><code>{tunnelReadiness.controlPlaneRoute.proxyUrl}</code></div>}
          </div>
        )}
      </Card>
      <RuntimeOverview />
    </div>
  );
}
