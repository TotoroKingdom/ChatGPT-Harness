import { useState } from "react";
import type { DiagnosticCheck, DiagnosticRun, DiagnosticScope, StartupAttempt } from "@protocol/v1";
import { useTranslation } from "react-i18next";
import { Button, InlineFeedback, StatusBadge } from "@/components/common/primitives";
import type { Navigate } from "@/app/router/routes";
import {
  diagnosticScopeKey,
  diagnosticStatusKey,
  startupOutcomeKey,
  startupPhaseKey,
  startupPhaseStatusKey,
} from "@/i18n/presentation";
import { formatDiagnosticCopy } from "@/services/harness/diagnostic-copy";

type Props = {
  run: DiagnosticRun | null;
  startup: StartupAttempt | null;
  running?: boolean;
  onRun: () => Promise<DiagnosticRun>;
  onNavigate?: Navigate;
};

const settingsTarget: Record<string, "workspace" | "connection" | "security"> = {
  settings_workspace: "workspace",
  settings_connection: "connection",
  settings_security: "security",
};

function slowestPhase(startup: StartupAttempt) {
  return startup.phases.reduce<StartupAttempt["phases"][number] | null>(
    (current, phase) => !current || phase.durationMs > current.durationMs ? phase : current,
    null,
  );
}

function remediationFor(check: DiagnosticCheck) {
  return check.remediationDetail ?? (check.remediation ? { code: "legacy", summary: check.remediation } : null);
}

export function DiagnosticsPanel({ run, startup, running = false, onRun, onNavigate }: Props) {
  const { t } = useTranslation();
  const [copyFeedback, setCopyFeedback] = useState<"copied" | "failed" | null>(null);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(formatDiagnosticCopy(run, startup));
      setCopyFeedback("copied");
    } catch {
      setCopyFeedback("failed");
    }
  };
  const tone = !run ? "neutral" : run.overallStatus === "pass" ? "success" : run.overallStatus === "fail" ? "danger" : "warning";
  const visual = !run ? "unknown" : run.overallStatus === "pass" ? "success" : run.overallStatus === "fail" ? "danger" : "warning";
  return <section className="diagnostics-summary" aria-label={t("diagnostics.title")}>
    <div className="diagnostics-summary-copy">
      <StatusBadge tone={tone} visual={visual}>{run ? t(diagnosticStatusKey(run.overallStatus)) : t("diagnostics.notRun")}</StatusBadge>
      <p>{run ? t("diagnostics.lastRun", { time: run.finishedAt }) : t("diagnostics.noRun")}</p>
    </div>
    <div className="diagnostics-summary-actions">
      <Button variant="primary" onClick={() => void onRun()} disabled={running}>{running ? t("diagnostics.running") : t("diagnostics.run")}</Button>
      {run && <Button variant="secondary" onClick={() => void copy()}>{t("diagnostics.copy")}</Button>}
    </div>
    {copyFeedback === "copied" && <InlineFeedback>{t("diagnostics.copied")}</InlineFeedback>}
    {copyFeedback === "failed" && <InlineFeedback error>{t("diagnostics.copyFailed")}</InlineFeedback>}
    {run && <details className="diagnostics-disclosure">
      <summary>
        <span>{t("diagnostics.checks")} · {run.checks.length}</span>
        <span className="muted">{t("diagnostics.expandDetails")}</span>
      </summary>
      <div className="diagnostic-check-list">
        {run.checks.map((check) => {
          const remediation = remediationFor(check);
          return <div className="diagnostic-check" key={check.id}>
            <div className="diagnostic-check-heading">
              <div><strong>{check.id}</strong><span>{t(diagnosticScopeKey((check.scope ?? "lifecycle") as DiagnosticScope))}</span></div>
              <StatusBadge tone={check.status === "pass" ? "success" : check.status === "fail" ? "danger" : "warning"} visual={check.status === "pass" ? "success" : check.status === "fail" ? "danger" : "warning"}>{t(diagnosticStatusKey(check.status))}</StatusBadge>
            </div>
            <p>{check.summary}</p>
            {check.evidence.length > 0 && <small>{t("diagnostics.evidence")}: {check.evidence.map((item) => `${item.key}=${item.value}`).join(", ")}</small>}
            {remediation && <div className="diagnostic-remediation"><span>{t("diagnostics.remediation")}: {remediation.summary}</span>{remediation.target && settingsTarget[remediation.target] && onNavigate && <Button variant="quiet" onClick={() => onNavigate("settings", { settingsSection: settingsTarget[remediation.target!] })}>{t("m8.openSettings")}</Button>}</div>}
          </div>;
        })}
      </div>
    </details>}
    {startup && <details className="diagnostics-disclosure">
      <summary><span>{t("diagnostics.startup")}</span><span className="muted">{t("diagnostics.expandDetails")}</span></summary>
      <div className="detail-list">
        <div><span>{t("diagnostics.cohort")}</span><span>{startup.cohort}</span></div>
        <div><span>{t("diagnostics.outcome")}</span><span>{t(startupOutcomeKey(startup.outcome))}</span></div>
        <div><span>{t("diagnostics.duration")}</span><span>{startup.totalDurationMs ?? "—"} {t("diagnostics.milliseconds")}</span></div>
        {slowestPhase(startup) && <div><span>{t("diagnostics.slowestPhase")}</span><span>{t(startupPhaseKey(slowestPhase(startup)!.phase))} · {slowestPhase(startup)!.durationMs} {t("diagnostics.milliseconds")}</span></div>}
        <strong className="detail-list-heading">{t("diagnostics.startupDetails")}</strong>
        {startup.phases.map((phase) => <div key={phase.phase}><span>{t(startupPhaseKey(phase.phase))}</span><span>{t(startupPhaseStatusKey(phase.status))} · {phase.durationMs} {t("diagnostics.milliseconds")}</span></div>)}
      </div>
    </details>}
  </section>;
}

