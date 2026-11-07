import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ConfigRevisionStatus } from "@/components/status/ConfigRevisionStatus";
import { Button, Disclosure, InlineFeedback, Inspector, ObservedTime, StatusBadge, type StatusTone, type StatusVisual } from "@/components/common/primitives";
import { DiagnosticsTrigger } from "@/components/status/DiagnosticsHost";
import { useHarnessData } from "@/app/providers/HarnessDataProvider";
import type { Navigate } from "@/app/router/routes";
import type { ComponentStatus } from "@/services/harness/client";
import { hasPolicyCritical } from "@/i18n/state-presentation";
import { activityEventKey, componentStatusKey, errorCodeKey, overallStatusKey, startEligibilityKey } from "@/i18n/presentation";
import { RuntimeOverview } from "./RuntimeOverview";

type Component = "runtime" | "tunnel" | "mcp";

function componentTone(status: ComponentStatus): StatusTone {
  if (status === "ready") return "success";
  if (status === "error") return "danger";
  if (status === "stopped") return "neutral";
  return "warning";
}

function serviceVisual(status: ComponentStatus): StatusVisual {
  return status;
}

function HeroGlyph({ tone }: { tone: StatusTone }) {
  if (tone === "success") {
    return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M7.5 12.2 10.5 15.2 16.5 8.8" /></svg>;
  }
  if (tone === "danger") {
    return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M8.5 8.5 15.5 15.5M15.5 8.5 8.5 15.5" /></svg>;
  }
  if (tone === "neutral") {
    return <svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9" /><path d="M8.7 9.2a3.4 3.4 0 1 1 5.5 2.7c-1.4.9-2.2 1.9-2.2 3.4M12 18.3v.2" /></svg>;
  }
  return <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3.2 21 20H3z" /><path d="M12 8.5v5.4M12 17.4v.2" /></svg>;
}

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
  const recovery = data.harness?.tunnelRecovery;
  const blocker = data.harness?.lastError?.code;
  const fix = () => onNavigate?.("settings", { settingsSection: (data.config && !data.config.workspace.rootPath) || data.harness?.lastError?.code === "WORKSPACE_INVALID" ? "workspace" : "connection" });
  const summaryKey = critical || blocker || failed
    ? "overview.summaryFailed"
    : !observed
      ? "overview.summaryUnknown"
      : transitioning
        ? "overview.summaryStarting"
        : stopped
          ? "overview.summaryStopped"
          : "overview.summaryReady";
  const heroTone: StatusTone = critical || blocker || failed ? "danger" : !observed ? "neutral" : transitioning ? "warning" : stopped ? "neutral" : "success";

  const run = async (fn: () => Promise<{ ok: boolean; error?: { message: string; code?: string } }>) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true);
    setActionError("");
    setMessage("");
    try {
      const result = await fn();
      if (!result.ok) setActionError(errorCodeKey(result.error));
      else setMessage("m8.lifecycleSubmitted");
    } catch (error) {
      setActionError(errorCodeKey(error));
    } finally {
      inFlight.current = false;
      setBusy(false);
    }
  };

  const refresh = () => {
    void data.refreshHarness?.();
    void data.refreshConfig?.();
  };

  const activityItems = data.activity?.status === "success" || data.activity?.status === "empty" ? data.activity.data.items : null;
  const pendingApprovalCount = data.approvals?.status === "success" || data.approvals?.status === "empty"
    ? data.approvals.data.items.filter((item) => item.status === "pending").length
    : undefined;

  return <div className="page-stack overview-page">
    <section className="overview-hero" aria-labelledby="overview-harness-status">
      <div className="overview-hero-main">
        <div className={`overview-hero-icon ${heroTone}`}><HeroGlyph tone={heroTone} /></div>
        <div className="overview-hero-copy">
          <h2 id="overview-harness-status">{critical ? t("m8.harnessNeedsAttention") : t("overview.harnessIs", { status: !observed ? t("m8.unknown") : t(overallStatusKey(data.overall)) })}</h2>
          <p>{t(summaryKey)}</p>
        </div>
      </div>
      <div className="overview-hero-actions">
        {critical ? <Button variant="primary" onClick={() => onNavigate?.("policies")}>{t("m8.reviewPolicies")}</Button>
          : !observed ? <Button variant="primary" onClick={refresh}>{t("common.retry")}</Button>
            : transitioning ? <Button disabled>{t(componentStatusKey(data.overall === "stopping" ? "stopping" : "starting"))}</Button>
              : failed ? <Button variant="primary" onClick={fix}>{t("m8.openSettings")}</Button>
                : data.restartRequired ? <Button variant="primary" onClick={() => void run(data.restartHarness)}>{t("overview.restart")}</Button>
                  : canStart ? <Button variant="primary" onClick={() => void run(data.startHarness)}>{t("overview.start")}</Button>
                    : stopped ? <Button variant="primary" onClick={fix}>{t("m8.openSettings")}</Button> : null}
        {canStop && <Button disabled={transitioning} onClick={() => void run(data.stopHarness)}>{t("overview.stop")}</Button>}
      </div>
    </section>

    {actionError && <InlineFeedback error>{t(actionError)}</InlineFeedback>}
    {message && <InlineFeedback>{t(message)}</InlineFeedback>}

    {critical ? <div className="notice notice-danger" role="alert"><strong>{t("policies.invalidSnapshot")}</strong><span>{t("m7.remediation.invalid_policy_snapshot")}</span></div>
      : blocker ? <div className="notice notice-danger" role="alert"><strong>{t(errorCodeKey(blocker))}</strong><span>{t("m8.checkConnection")}</span></div>
        : tunnelReadiness && !tunnelReadiness.controlPlaneConnected ? <div className="notice notice-warning" role="alert">{t(errorCodeKey("CONTROL_PLANE_UNAVAILABLE"))}</div>
          : (stopped || failed) && data.eligibility && !data.eligibility.canStart ? <div className="notice notice-warning" role="alert"><strong>{t(startEligibilityKey(data.eligibility.status))}</strong><span>{t(`overview.eligibility.${data.eligibility.status}`)}</span></div>
            : data.restartRequired ? <InlineFeedback>{t("overview.restartRequired")} · {t("overview.configChanged")}</InlineFeedback> : null}

    {recovery && !["disarmed", "idle"].includes(recovery.phase) && <div className="notice notice-warning" role="status">
      <strong>{t("overview.tunnelRecovery")}: {t(`overview.recoveryPhase.${recovery.phase}`)}</strong>
      {recovery.reason && <span>{recovery.reason.code}: {recovery.reason.message}</span>}
      {recovery.cooldownUntil && <span>{t("overview.cooldownUntil")}: <ObservedTime value={recovery.cooldownUntil} fallback={recovery.cooldownUntil} /></span>}
      {recovery.nextAttemptAt && <span>{t("overview.nextRecoveryAttempt")}: <ObservedTime value={recovery.nextAttemptAt} fallback={recovery.nextAttemptAt} /></span>}
    </div>}

    <div className="overview-columns">
      <section className="overview-panel" aria-labelledby="overview-services-heading">
        <div className="overview-panel-head">
          <div><h2 id="overview-services-heading">{t("overview.services")}</h2><span className="muted">{t("overview.serviceRuntime")} · {t("overview.serviceTunnel")} · {t("overview.serviceMcp")}</span></div>
        </div>
        <div className="overview-panel-body">
          <div className="overview-services">
            {(["runtime", "tunnel", "mcp"] as const).map((component) => {
              const label = component === "mcp" ? "MCP" : component === "runtime" ? "Runtime" : "Tunnel";
              const status = data.components[component];
              const statusText = observed ? t(componentStatusKey(status)) : t("m8.unknown");
              return <button type="button" key={component} className="service-card" onClick={() => setSelected(component)} aria-label={`${label} ${statusText}`}>
                <span className="service-card-label"><strong>{label}</strong><StatusBadge tone={observed ? componentTone(status) : "neutral"} visual={observed ? serviceVisual(status) : "unknown"}>{statusText}</StatusBadge></span>
                <span>{component === "runtime" ? t("overview.serviceRuntime") : component === "tunnel" ? t("overview.serviceTunnel") : t("overview.serviceMcp")}</span>
              </button>;
            })}
          </div>
          <div className="overview-context">
            <div><span>{t("settings.workspace.root")}</span><strong className="mono">{data.config?.workspace.rootPath || t("m8.unknown")}</strong></div>
            <button type="button" onClick={() => onNavigate?.("approvals")}><span>{t("m8Approvals.pending")}</span><strong>{pendingApprovalCount ?? "—"}</strong></button>
          </div>
        </div>
      </section>

      <section className="overview-panel" aria-labelledby="overview-activity-heading">
        <div className="overview-panel-head">
          <div><h2 id="overview-activity-heading">{t("m8.recentActivity")}</h2><span className="muted">{t("overview.recentActivityHint")}</span></div>
          <Button variant="quiet" onClick={() => onNavigate?.("activity")}>{t("nav.activity")}</Button>
        </div>
        <div className="overview-panel-body">
          {activityItems === null ? <p className="muted overview-empty-copy">{t("m8.activityUnconfirmed")}</p>
            : activityItems.length === 0 ? <p className="muted overview-empty-copy">{t("activity.empty")}</p>
              : activityItems.slice(-4).reverse().map((event, index) => <div className="activity-mini-row" key={event.eventId ?? index}>
                <span className="event-marker success" aria-hidden="true" />
                <div className="activity-mini-main"><strong>{t(activityEventKey(event.eventType))}</strong><ObservedTime value={event.timestamp} fallback={event.timestamp} /></div>
              </div>)}
        </div>
      </section>
    </div>

    <div className="overview-diagnostics-cta">
      <div><strong>{t("diagnostics.title")}</strong><p>{t("overview.diagnosticsPrompt")}</p></div>
      <DiagnosticsTrigger variant="secondary" />
    </div>

    <Disclosure title={t("m8.advanced")}><ConfigRevisionStatus /><RuntimeOverview /></Disclosure>

    {selected && <Inspector title={selected === "mcp" ? "MCP" : selected === "runtime" ? "Runtime" : "Tunnel"} onClose={() => setSelected(null)}>
      <StatusBadge tone={observed ? componentTone(data.components[selected]) : "neutral"} visual={observed ? serviceVisual(data.components[selected]) : "unknown"}>{observed ? t(componentStatusKey(data.components[selected])) : t("m8.unknown")}</StatusBadge>
      <p className="settings-help">{t("overview.authority")}</p>
      <Disclosure title={t("m8.advanced")}><ConfigRevisionStatus />
        {selected === "tunnel" && recovery && <pre>{JSON.stringify({ recovery }, null, 2)}</pre>}
        {selected === "tunnel" && tunnelReadiness && <pre>{JSON.stringify({ health: data.harness?.tunnelHealth, controlPlaneConnected: tunnelReadiness.controlPlaneConnected, mcpTargetReachable: tunnelReadiness.mcpTargetReachable, healthCheckPassed: tunnelReadiness.healthCheckPassed, lastSuccessfulPollTimestamp: tunnelReadiness.lastSuccessfulPollTimestamp, lastSuccessfulPollAgeSeconds: tunnelReadiness.lastSuccessfulPollAgeSeconds, controlPlaneError: tunnelReadiness.controlPlaneError, controlPlaneRoute: tunnelReadiness.controlPlaneRoute }, null, 2)}</pre>}
        {data.harness?.lastError && <code>{data.harness.lastError.code}</code>}
      </Disclosure>
    </Inspector>}
  </div>;
}
