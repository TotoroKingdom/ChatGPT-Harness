import { ConfigRevisionStatus } from "@/components/status/ConfigRevisionStatus";
import { useRef, useState } from "react";
import { Button, Disclosure, InlineFeedback, Inspector, Row, Section, StatusBadge } from "@/components/common/primitives";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { Navigate } from "@/app/router/routes";
import { RuntimeOverview } from "./RuntimeOverview";
import { useTranslation } from "react-i18next";
import { componentStatusKey, errorCodeKey, overallStatusKey, startEligibilityKey, activityEventKey } from "@/i18n/presentation";

import { hasPolicyCritical } from "@/i18n/state-presentation";

type Component = "runtime" | "tunnel" | "mcp";
export function OverviewPage({ onNavigate }: { onNavigate?: Navigate }) {
  const { t } = useTranslation();
  const data = useHarnessData();
  const [message, setMessage] = useState("");
  const [actionError, setActionError] = useState("");
  const [busy, setBusy] = useState(false);
  const inFlight = useRef(false);
  const [selected, setSelected] = useState<Component | null>(null);
  const observed = data.harnessObserved !== false;
  const critical = data.policies ? hasPolicyCritical(data.components.runtime, data.policies) : false;
  const transitioning = busy || data.overall === "starting" || data.overall === "stopping";
  const stopped = data.overall === "stopped";
  const failed = data.overall === "error" || data.overall === "degraded";
  const canStart = observed && (stopped || data.overall === "error") && Boolean(data.eligibility?.canStart);
  const canStop = observed && !stopped && (Boolean(data.harness?.activeSessionId) || Object.values(data.components).some((state) => state !== "stopped"));
  const tunnelReadiness = data.harness?.tunnelReadiness;
  const fix = () => onNavigate?.("settings", { settingsSection: (data.config && !data.config.workspace.rootPath) || data.harness?.lastError?.code === "WORKSPACE_INVALID" ? "workspace" : "connection" });
  const run = async (fn: () => Promise<{ ok: boolean; error?: { message: string; code?: string } }>) => {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setActionError(""); setMessage("");
    try {
      const result = await fn();
      if (!result.ok) setActionError(errorCodeKey(result.error));
      else setMessage("m8.lifecycleSubmitted");
    } catch (error) { setActionError(errorCodeKey(error)); }
    finally { inFlight.current = false; setBusy(false); }
  };
  const refresh = () => { void data.refreshHarness?.(); void data.refreshConfig?.(); };
  const blocker = data.harness?.lastError?.code;
  return <div className="page-stack">
    <div className="workspace-row"><div className="row-identity"><h2>{critical ? t("m8.harnessNeedsAttention") : t("overview.harnessIs", { status: observed ? t(overallStatusKey(data.overall)) : t("m8.unknown") })}</h2><p>{t("m8.overviewTask")}</p></div>
      <div className="row-controls">
        {critical ? <Button variant="primary" onClick={() => onNavigate?.("policies")}>{t("m8.reviewPolicies")}</Button> : !observed ? <Button variant="primary" onClick={refresh}>{t("common.retry")}</Button>
        : transitioning ? <Button disabled>{t(componentStatusKey(data.overall === "stopping" ? "stopping" : "starting"))}</Button>
        : failed ? <Button variant="primary" onClick={fix}>{t("m8.openSettings")}</Button>
        : data.restartRequired ? <Button variant="primary" onClick={() => void run(data.restartHarness)}>{t("overview.restart")}</Button>
        : canStart ? <Button variant="primary" onClick={() => void run(data.startHarness)}>{t("overview.start")}</Button>
        : stopped ? <Button variant="primary" onClick={fix}>{t("m8.openSettings")}</Button> : null}
        {canStop && <Button disabled={transitioning} onClick={() => void run(data.stopHarness)}>{t("overview.stop")}</Button>}
        {failed && canStart && !canStop && <Button disabled={transitioning} onClick={() => void run(data.startHarness)}>{t("overview.start")}</Button>}
      </div>
    </div>
    {actionError && <InlineFeedback error>{t(actionError)}</InlineFeedback>}
    {message && <InlineFeedback>{t(message)}</InlineFeedback>}
    {critical ? <div className="notice notice-danger" role="alert"><strong>{t("policies.invalidSnapshot")}</strong><span>{t("m7.remediation.invalid_policy_snapshot")}</span></div> : blocker ? <div className="notice notice-danger" role="alert"><strong>{t(errorCodeKey(blocker))}</strong><span>{t("m8.checkConnection")}</span></div>
      : tunnelReadiness && !tunnelReadiness.controlPlaneConnected ? <div className="notice notice-warning" role="alert">{t(errorCodeKey("CONTROL_PLANE_UNAVAILABLE"))}</div>
      : (stopped || failed) && data.eligibility && !data.eligibility.canStart ? <div className="notice notice-warning" role="alert"><strong>{t(startEligibilityKey(data.eligibility.status))}</strong><span>{t(`overview.eligibility.${data.eligibility.status}`)}</span></div>
      : data.restartRequired ? <InlineFeedback>{t("overview.restartRequired")} · {t("overview.configChanged")}</InlineFeedback> : null}
    <Section title={t("overview.services")}>
      {(["runtime", "tunnel", "mcp"] as const).map((component) => <Row key={component} title={component === "mcp" ? "MCP" : component === "runtime" ? "Runtime" : "Tunnel"} selected={selected === component} onInspect={() => setSelected(component)} controls={<StatusBadge>{observed ? t(componentStatusKey(data.components[component])) : t("m8.unknown")}</StatusBadge>} />)}
    </Section>
    <Section title={t("m8.recentActivity")}>
      {data.activity?.status === "success" || data.activity?.status === "empty" ? data.activity.data.items.length === 0 ? <p>{t("activity.empty")}</p> : data.activity.data.items.slice(-3).reverse().map((event, index) => <Row key={event.eventId ?? index} title={t(activityEventKey(event.eventType))} summary={event.timestamp} />) : <p>{t("m8.activityUnconfirmed")}</p>}
    </Section>
    <Disclosure title={t("m8.advanced")}><ConfigRevisionStatus /><RuntimeOverview /></Disclosure>
    {selected && <Inspector title={selected === "mcp" ? "MCP" : selected === "runtime" ? "Runtime" : "Tunnel"} onClose={() => setSelected(null)}>
      <StatusBadge>{observed ? t(componentStatusKey(data.components[selected])) : t("m8.unknown")}</StatusBadge><p>{t("overview.authority")}</p>
      <Disclosure title={t("m8.advanced")}><ConfigRevisionStatus />
        {selected === "tunnel" && tunnelReadiness && <pre>{JSON.stringify({ controlPlaneConnected: tunnelReadiness.controlPlaneConnected, mcpTargetReachable: tunnelReadiness.mcpTargetReachable, healthCheckPassed: tunnelReadiness.healthCheckPassed, lastSuccessfulPollTimestamp: tunnelReadiness.lastSuccessfulPollTimestamp, lastSuccessfulPollAgeSeconds: tunnelReadiness.lastSuccessfulPollAgeSeconds, controlPlaneError: tunnelReadiness.controlPlaneError, controlPlaneRoute: tunnelReadiness.controlPlaneRoute }, null, 2)}</pre>}
        {data.harness?.lastError && <code>{data.harness.lastError.code}</code>}
      </Disclosure>
    </Inspector>}
  </div>;
}
