import type { DiagnosticRun, StartupAttempt } from "@protocol/v1";

const evidenceKeys = new Set([
  "configured",
  "exists",
  "version",
  "process_phase",
  "lifecycle",
  "readiness",
  "route_mode",
  "proxy_source",
  "last_success_age_seconds",
  "error_code",
  "endpoint_reachable",
]);

function safeValue(value: string): string {
  if (value.includes("\\") || value.includes("/") || value.includes("://")) return "<redacted>";
  return value.replace(/[\u0000-\u001f\u007f]/g, "").slice(0, 128);
}

/**
 * Strict allowlist formatter. Do not add arbitrary DTO fields here: future
 * fields must be explicitly reviewed before they can enter clipboard output.
 */
export function formatDiagnosticCopy(run: DiagnosticRun | null, startup: StartupAttempt | null): string {
  if (!run) return "";
  const lines = [`overall=${run.overallStatus}`];
  for (const check of run.checks) {
    const scope = check.scope ?? "unknown";
    const evidence = check.evidence
      .filter((item) => evidenceKeys.has(item.key))
      .map((item) => `${item.key}=${safeValue(item.value)}`)
      .join(",");
    const remediation = check.remediation ? ` remediation=${safeValue(check.remediation.code)}` : "";
    lines.push(`check=${safeValue(check.id)} status=${check.status} scope=${scope} evidence=${evidence}${remediation}`);
  }
  if (startup) {
    lines.push(`startup cohort=${startup.cohort} outcome=${startup.outcome} totalDurationMs=${startup.totalDurationMs ?? "unknown"}`);
    for (const phase of startup.phases) {
      lines.push(`phase=${phase.phase} status=${phase.status} durationMs=${phase.durationMs}`);
    }
  }
  return lines.join("\n");
}

