import { useState } from "react";
import type { DiagnosticCheck, DiagnosticRun, DiagnosticScope, StartupAttempt } from "@protocol/v1";
import { useTranslation } from "react-i18next";
import { Button, InlineFeedback, Row, Section, StatusBadge } from "@/components/common/primitives";
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
  return <Section title={t("diagnostics.run")}>
    <div className="settings-actions">
      <Button variant="primary" onClick={() => void onRun()} disabled={running}>{running ? t("diagnostics.running") : t("diagnostics.run")}</Button>
      {run && <Button variant="secondary" onClick={() => void copy()}>{t("diagnostics.copy")}</Button>}
    </div>
    {copyFeedback === "copied" && <InlineFeedback>{t("diagnostics.copied")}</InlineFeedback>}
    {copyFeedback === "failed" && <InlineFeedback error>{t("diagnostics.copyFailed")}</InlineFeedback>}
    {!run && <p className="muted">{t("diagnostics.noRun")}</p>}
    {run && <>
      <Row title={t("diagnostics.overall")} controls={<StatusBadge>{t(diagnosticStatusKey(run.overallStatus))}</StatusBadge>} />
      <div className="detail-list">
        {run.checks.map((check) => <div key={check.id}>
          <div className="row-identity"><strong>{check.id}</strong><span className="muted">{t(diagnosticScopeKey((check.scope ?? "lifecycle") as DiagnosticScope))}</span></div>
          <div className="settings-actions"><StatusBadge>{t(diagnosticStatusKey(check.status))}</StatusBadge><span>{check.summary}</span></div>
          {check.evidence.length > 0 && <small className="muted">{t("diagnostics.evidence")}: {check.evidence.map((item) => `${item.key}=${item.value}`).join(", ")}</small>}
          {remediationFor(check) && <div className="settings-actions"><span className="muted">{t("diagnostics.remediation")}: {remediationFor(check)!.summary}</span>{remediationFor(check)!.target && settingsTarget[remediationFor(check)!.target!] && onNavigate && <Button variant="quiet" onClick={() => onNavigate("settings", { settingsSection: settingsTarget[remediationFor(check)!.target!] })}>{t("m8.openSettings")}</Button>}</div>}
        </div>)}
      </div>
    </>}
    {startup && <div className="detail-list">
      <strong>{t("diagnostics.startup")}</strong>
      <div><span>{t("diagnostics.cohort")}</span><span>{startup.cohort}</span></div>
      <div><span>{t("diagnostics.outcome")}</span><span>{t(startupOutcomeKey(startup.outcome))}</span></div>
      <div><span>{t("diagnostics.duration")}</span><span>{startup.totalDurationMs ?? "—"} {t("diagnostics.milliseconds")}</span></div>
      {slowestPhase(startup) && <div><span>{t("diagnostics.slowestPhase")}</span><span>{t(startupPhaseKey(slowestPhase(startup)!.phase))} · {slowestPhase(startup)!.durationMs} {t("diagnostics.milliseconds")}</span></div>}
      <strong>{t("diagnostics.startupDetails")}</strong>
      {startup.phases.map((phase) => <div key={phase.phase}><span>{t(startupPhaseKey(phase.phase))}</span><span>{t(startupPhaseStatusKey(phase.status))} · {phase.durationMs} {t("diagnostics.milliseconds")}</span></div>)}
    </div>}
  </Section>;
}

